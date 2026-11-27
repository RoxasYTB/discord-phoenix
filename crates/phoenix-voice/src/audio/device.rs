use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, Stream, StreamConfig, SupportedStreamConfig};
use tokio::sync::mpsc;

use super::dsp::{NoiseGate, Resampler, convert_channels, level_db, to_i16};
use super::processing::{ChainSettings, InputChain, KrispGate, RenderFeed, RenderTap, SyntheticNoisyVoice, render_channel};
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
        let (tap, feed) = render_channel();
        let bypass = input.as_deref() == Some(SYNTHETIC_INPUT);

        let mut threads = Vec::new();
        let oq = out_queue.clone();
        let output_name = output.clone();
        threads.push(
            std::thread::Builder::new()
                .name("phoenix-audio-io".into())
                .spawn(move || {
                    let synthetic = matches!(input.as_deref(), Some(SYNTHETIC_INPUT | SYNTHETIC_VOICE_INPUT));
                    let tone_stop = Arc::new(AtomicBool::new(false));
                    let (istream, ifmt) = if synthetic {
                        let ts = tone_stop.clone();
                        let voice = input.as_deref() == Some(SYNTHETIC_VOICE_INPUT);
                        let _ = std::thread::Builder::new().name("phoenix-audio-tone".into()).spawn(move || synthetic_source(cap_tx, ts, voice));
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

        if let Some((rate, _)) = ifmt {
            let control = control.clone();
            let stop = stop.clone();
            threads.push(
                std::thread::Builder::new()
                    .name("phoenix-audio-enc".into())
                    .spawn(move || encode_loop(control, stop, cap_rx, rate, out_tx, Processor::new(Some(feed), bypass)))
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
                    .spawn(move || mix_loop(control, stop, mixer, out_queue, rate, ch, tap))
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
pub const SYNTHETIC_VOICE_INPUT: &str = "phoenix-synthetic-noisy-voice";

pub fn synthetic_frame(start: usize, len: usize) -> Vec<f32> {
    (start..start + len).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin() * 0.03).collect()
}

fn synthetic_source(tx: std_mpsc::Sender<Vec<f32>>, stop: Arc<AtomicBool>, voice: bool) {
    let mut n = 0usize;
    let mut next = Instant::now();
    let mut source = voice.then(SyntheticNoisyVoice::new);
    while !stop.load(Ordering::Relaxed) {
        let frame = match source.as_mut() {
            Some(g) => {
                let mut f = vec![0.0; FRAME_SAMPLES];
                g.fill(&mut f);
                f
            }
            None => synthetic_frame(n, FRAME_SAMPLES),
        };
        if tx.send(frame).is_err() {
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
    chain: InputChain,
    gate: NoiseGate,
    krisp_gate: KrispGate,
    bypass: bool,
    ptt_last: Option<Instant>,
}

impl Default for Processor {
    fn default() -> Self {
        Self::new(None, false)
    }
}

impl Processor {
    pub fn new(feed: Option<RenderFeed>, bypass: bool) -> Self {
        Self { chain: InputChain::new(ChainSettings::OFF, feed), gate: NoiseGate::default(), krisp_gate: KrispGate::new(), bypass, ptt_last: None }
    }

    pub fn chain(&self) -> &InputChain {
        &self.chain
    }

    pub fn process(&mut self, control: &AudioControl, frame: &mut [f32]) -> (f32, bool) {
        let g = control.input_gain();
        if g != 1.0 {
            frame.iter_mut().for_each(|s| *s *= g);
        }
        let settings = if self.bypass { ChainSettings::OFF } else { ChainSettings::from_control(control) };
        self.chain.configure(settings);
        let krisp_vad = self.chain.process(frame);
        let db = level_db(frame);
        self.gate.automatic = control.gate_automatic();
        self.gate.threshold_db = control.gate_threshold_db();
        let level_open = self.gate.process(db);
        let vad = match krisp_vad {
            Some(p) if control.vad_use_krisp() && control.gate_automatic() => self.krisp_gate.process(p, KrispGate::threshold(settings.agc)),
            _ => level_open,
        };
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
        (db, open)
    }
}

fn encode_loop(
    control: Arc<AudioControl>,
    stop: Arc<AtomicBool>,
    rx: std_mpsc::Receiver<Vec<f32>>,
    rate: u32,
    out: mpsc::UnboundedSender<AudioOut>,
    mut proc: Processor,
) {
    let Ok(mut enc) = OpusEncoder::new(false, 64_000) else {
        tracing::warn!("opus encoder unavailable");
        return;
    };
    if let Err(e) = enc.enable_fec(10) {
        tracing::debug!(error = %e, "opus fec");
    }
    let mut rs = Resampler::new(rate, RATE, 1);
    let mut pending: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES * 4);
    let log = std::env::var_os("PHOENIX_SOUND_LOG").is_some();
    let mut last_log = Instant::now();
    let (mut sent, mut frames, mut in_db_sum) = (0u64, 0u64, 0f64);
    while !stop.load(Ordering::Relaxed) {
        let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
        rs.process(&chunk, &mut pending);
        while pending.len() >= FRAME_SAMPLES {
            let mut frame: Vec<f32> = pending.drain(..FRAME_SAMPLES).collect();
            if log {
                in_db_sum += f64::from(level_db(&frame));
            }
            let (db, open) = proc.process(&control, &mut frame);
            control.set_input_level_db(db);
            let send = open && !control.muted() && !control.deafened() && !control.mic_test_active();
            control.set_transmitting(send);
            frames += 1;
            sent += u64::from(send);
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
            if log && last_log.elapsed() >= Duration::from_secs(5) {
                last_log = Instant::now();
                let c = proc.chain();
                let st = c.stats();
                eprintln!(
                    "voice-proc: chain={} in_db={:.1} out_db={db:.1} open={sent}/{frames} cpu_avg_us={:.0} cpu_max_us={:.0} load={:.2}% latency_ms={:.1} render_blocks={} erle={:?}",
                    c.settings().label(),
                    in_db_sum / frames.max(1) as f64,
                    st.avg_block_us(),
                    st.max_block.as_secs_f64() * 1e6,
                    st.load_pct(),
                    c.latency_samples() as f64 * 1000.0 / RATE as f64,
                    st.render_blocks,
                    c.echo_return_loss_enhancement(),
                );
                (sent, frames, in_db_sum) = (0, 0, 0.0);
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
    _pause: Option<MicTestPause>,
}

struct MicTestPause(Arc<AudioControl>);

impl MicTestPause {
    fn new(control: Arc<AudioControl>) -> Self {
        control.mic_tests.fetch_add(1, Ordering::Relaxed);
        Self(control)
    }
}

impl Drop for MicTestPause {
    fn drop(&mut self) {
        self.0.mic_tests.fetch_sub(1, Ordering::Relaxed);
    }
}

impl MicTest {
    pub fn start(control: Arc<AudioControl>, input: Option<String>, output: Option<String>, loopback: bool) -> Result<Self> {
        let pause = loopback.then(|| MicTestPause::new(control.clone()));
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
        let (irate, ofmt) = match (ifmt, ofmt) {
            (Some((rate, _)), output) if output.is_some() || !loopback => (rate, output),
            (input, _) => {
                drop(io_stop_tx);
                for t in threads {
                    let _ = t.join();
                }
                let missing = if input.is_none() { "no microphone" } else { "no speakers" };
                return Err(Error::Unsupported(missing.into()));
            }
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
        Ok(Self { stop, io_stop: Some(io_stop_tx), threads, level, open, _pause: pause })
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
    tap: RenderTap,
) {
    let mut rs = Resampler::new(RATE, rate, ch);
    let cap = rate as usize * ch * MAX_OUT_MS / 1000;
    let target = rate as usize * ch * OUT_TARGET_MS / 1000;
    let log = std::env::var_os("PHOENIX_SOUND_LOG").is_some();
    let mut last_log = Instant::now();
    let mut underruns = 0u64;
    let mut buf = Vec::with_capacity(FRAME_SAMPLES * 4);
    let effects = super::effects::EngineOutput::attach();
    while !stop.load(Ordering::Relaxed) {
        let queued = lock(&q).len();
        if queued == 0 {
            underruns += 1;
        }
        let mut pending = queued;
        while pending < target && !stop.load(Ordering::Relaxed) {
            let mut mixed = lock(&mixer).mix_frame(&control);
            effects.mix_into(&mut mixed);
            let ahead_ms = (pending * 1000 / (rate as usize * ch).max(1)) as u32;
            let frame = convert_channels(&mixed, 2, ch);
            buf.clear();
            rs.process(&frame, &mut buf);
            let played = {
                let mut q = lock(&q);
                let fits = q.len() + buf.len() <= cap;
                if fits {
                    q.extend(buf.iter().copied());
                }
                pending = q.len();
                fits
            };
            if played {
                tap.push(&mixed, 2, ahead_ms);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mic_test_pauses_call_audio_until_the_last_test_stops() {
        let control = Arc::new(AudioControl::default());
        control.set_gate(Some(-100.0));
        let stop = Arc::new(AtomicBool::new(false));
        let (capture_tx, capture_rx) = std_mpsc::channel();
        let (voice_tx, mut voice_rx) = mpsc::unbounded_channel();
        let (encoder_control, encoder_stop) = (control.clone(), stop.clone());
        let encoder = std::thread::spawn(move || {
            encode_loop(encoder_control, encoder_stop, capture_rx, RATE, voice_tx, Processor::new(None, true));
        });
        let mut next_voice_frame = async || {
            capture_tx.send(synthetic_frame(0, FRAME_SAMPLES)).unwrap();
            tokio::time::timeout(Duration::from_secs(5), voice_rx.recv()).await.expect("voice encoder timed out").unwrap()
        };

        let AudioOut::Frame(opus) = next_voice_frame().await else { panic!("expected an encoded voice frame") };
        let mut mixer = Mixer::default();
        for sequence in 0..4 {
            mixer.push(AudioIn { user_id: 1, ssrc: 1, sequence, timestamp: u32::from(sequence) * FRAME_SAMPLES as u32, opus: opus.clone() });
        }
        assert!(mixer.mix_frame(&control).iter().any(|s| *s != 0.0));

        let test = MicTestPause::new(control.clone());
        let second_test = MicTestPause::new(control.clone());
        assert!(matches!(next_voice_frame().await, AudioOut::End));
        assert!(mixer.mix_frame(&control).iter().all(|s| *s == 0.0));
        drop(test);
        assert!(matches!(next_voice_frame().await, AudioOut::End));
        assert!(mixer.mix_frame(&control).iter().all(|s| *s == 0.0));
        drop(second_test);
        assert!(matches!(next_voice_frame().await, AudioOut::Frame(_)));
        assert!(mixer.mix_frame(&control).iter().any(|s| *s != 0.0));

        stop.store(true, Ordering::Relaxed);
        drop(capture_tx);
        encoder.join().unwrap();
    }
}
