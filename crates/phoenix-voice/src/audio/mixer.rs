use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::AudioControl;
use super::dsp::{level_db, mix_into};
use crate::FRAME_SAMPLES;
use crate::connection::AudioIn;
use crate::media::AudioDecoder;
use crate::media::opus::OpusDecoder;
use crate::rtp::jitter::{JitterBuffer, JitterOutput, JitterPacket};

const PREBUFFER: usize = 2;
const MAX_QUEUE: usize = 10;
const MAX_PLC: u32 = 3;
const IDLE_DROP: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TrackStats {
    pub ssrc: u32,
    pub received: u64,
    pub late: u64,
    pub decoded: u64,
    pub concealed: u64,
    pub recovered: u64,
    pub dropped: u64,
    pub resets: u64,
    pub level_db: f32,
    pub peak_db: f32,
}

struct Track {
    ssrc: u32,
    jitter: JitterBuffer,
    decoder: OpusDecoder,
    playing: bool,
    plc_run: u32,
    conceal_left: u16,
    last_push: Instant,
    stats: TrackStats,
}

impl Track {
    fn new(ssrc: u32) -> Option<Self> {
        Some(Self {
            ssrc,
            jitter: JitterBuffer::new(4, 64),
            decoder: OpusDecoder::new(true).ok()?,
            playing: false,
            plc_run: 0,
            conceal_left: 0,
            last_push: Instant::now(),
            stats: TrackStats { ssrc, level_db: -100.0, peak_db: -100.0, ..Default::default() },
        })
    }

    fn restart(&mut self, ssrc: u32) {
        self.ssrc = ssrc;
        self.stats.ssrc = ssrc;
        self.stats.resets += 1;
        self.jitter.reset();
        if let Ok(d) = OpusDecoder::new(true) {
            self.decoder = d;
        }
        self.playing = false;
        self.plc_run = 0;
        self.conceal_left = 0;
    }

    fn conceal(&mut self) -> Option<Vec<i16>> {
        self.stats.concealed += 1;
        self.decoder.decode(None).ok()
    }

    fn recover(&mut self) -> Option<Vec<i16>> {
        let next = self.jitter.peek().map(|p| p.payload.clone());
        match next {
            Some(p) => match self.decoder.decode_fec(&p) {
                Ok(pcm) => {
                    self.stats.recovered += 1;
                    Some(pcm)
                }
                Err(_) => self.conceal(),
            },
            None => self.conceal(),
        }
    }

    fn next_frame(&mut self) -> Option<Vec<i16>> {
        if self.conceal_left > 0 {
            self.conceal_left -= 1;
            return if self.conceal_left == 0 { self.recover() } else { self.conceal() };
        }
        if !self.playing {
            if self.jitter.len() < PREBUFFER {
                return None;
            }
            self.playing = true;
        }
        while self.jitter.len() > MAX_QUEUE {
            if self.jitter.pop().is_some() {
                self.stats.dropped += 1;
            }
        }
        match self.jitter.pop() {
            Some(JitterOutput::Packet(p)) => {
                self.plc_run = 0;
                match self.decoder.decode(Some(&p.payload)) {
                    Ok(pcm) => {
                        self.stats.decoded += 1;
                        Some(pcm)
                    }
                    Err(_) => self.conceal(),
                }
            }
            Some(JitterOutput::Lost { count, .. }) => {
                self.plc_run = 0;
                let n = count.clamp(1, MAX_PLC as u16);
                self.stats.dropped += u64::from(count.saturating_sub(n));
                self.conceal_left = n - 1;
                if self.conceal_left == 0 { self.recover() } else { self.conceal() }
            }
            None if self.jitter.skip_to_available() => {
                self.plc_run = 0;
                self.recover()
            }
            None => {
                self.plc_run += 1;
                if self.plc_run > MAX_PLC {
                    self.playing = false;
                    return None;
                }
                self.conceal()
            }
        }
    }
}

pub struct Mixer {
    tracks: HashMap<u64, Track>,
    output_level_db: f32,
    output_peak_db: f32,
}

impl Default for Mixer {
    fn default() -> Self {
        Self { tracks: HashMap::new(), output_level_db: -100.0, output_peak_db: -100.0 }
    }
}

impl Mixer {
    pub fn push(&mut self, a: AudioIn) {
        let t = match self.tracks.entry(a.user_id) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => match Track::new(a.ssrc) {
                Some(t) => v.insert(t),
                None => return,
            },
        };
        if t.ssrc != a.ssrc {
            t.restart(a.ssrc);
        }
        t.last_push = Instant::now();
        t.stats.received += 1;
        if !t.jitter.push(JitterPacket { sequence: a.sequence, timestamp: a.timestamp, marker: false, payload: a.opus }) {
            t.stats.late += 1;
        }
    }

    pub fn remove_user(&mut self, user: u64) {
        self.tracks.remove(&user);
    }

    pub fn stats(&self) -> Vec<(u64, TrackStats)> {
        let mut v: Vec<_> = self.tracks.iter().map(|(u, t)| (*u, t.stats)).collect();
        v.sort_by_key(|(u, _)| *u);
        v
    }

    pub fn output_level_db(&self) -> f32 {
        self.output_level_db
    }

    pub fn output_peak_db(&self) -> f32 {
        self.output_peak_db
    }

    pub fn reset_peaks(&mut self) {
        self.output_peak_db = -100.0;
        for t in self.tracks.values_mut() {
            t.stats.peak_db = -100.0;
        }
    }

    pub fn mix_frame(&mut self, control: &AudioControl) -> Vec<f32> {
        let mut acc = vec![0f32; FRAME_SAMPLES * 2];
        let now = Instant::now();
        self.tracks.retain(|_, t| now.duration_since(t.last_push) < IDLE_DROP);
        let deaf = control.deafened() || control.mic_test_active();
        for (uid, t) in self.tracks.iter_mut() {
            let Some(pcm) = t.next_frame() else {
                t.stats.level_db = -100.0;
                continue;
            };
            let gain = if deaf { 0.0 } else { control.track_gain(*uid) };
            let mut track = vec![0f32; pcm.len()];
            mix_into(&mut track, &pcm, 1.0);
            t.stats.level_db = level_db(&track);
            t.stats.peak_db = t.stats.peak_db.max(t.stats.level_db);
            if gain > 0.0 {
                mix_into(&mut acc, &pcm, gain);
            }
        }
        let g = control.output_gain();
        acc.iter_mut().for_each(|s| *s = (*s * g).clamp(-1.0, 1.0));
        self.output_level_db = level_db(&acc);
        self.output_peak_db = self.output_peak_db.max(self.output_level_db);
        acc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::AudioEncoder;
    use crate::media::opus::OpusEncoder;

    fn tone(freq: f32, amp: f32) -> Vec<i16> {
        (0..FRAME_SAMPLES * 2).map(|i| ((i / 2) as f32 * freq * std::f32::consts::TAU / 48_000.0).sin() * amp).map(|s| s as i16).collect()
    }

    fn packet(enc: &mut OpusEncoder, pcm: &[i16], user: u64, ssrc: u32, seq: u16) -> AudioIn {
        AudioIn { user_id: user, ssrc, sequence: seq, timestamp: seq as u32 * 960, opus: enc.encode(pcm).unwrap() }
    }

    fn energy(f: &[f32]) -> f32 {
        f.iter().map(|s| s * s).sum()
    }

    #[test]
    fn mixes_after_prebuffer_and_respects_deafen() {
        let mut enc = OpusEncoder::new(true, 64_000).unwrap();
        let pcm = tone(440.0, 12000.0);
        let c = AudioControl::default();
        let mut m = Mixer::default();
        m.push(packet(&mut enc, &pcm, 7, 1, 0));
        assert!(m.mix_frame(&c).iter().all(|s| *s == 0.0));
        for s in 1..6 {
            m.push(packet(&mut enc, &pcm, 7, 1, s));
        }
        let mut e = 0.0;
        for _ in 0..4 {
            e += energy(&m.mix_frame(&c));
        }
        assert!(e > 1.0);
        assert!(m.output_level_db() > -40.0);
        c.set_deafened(true);
        assert!(m.mix_frame(&c).iter().all(|s| *s == 0.0));
        assert!(m.stats()[0].1.level_db > -40.0);
        for _ in 0..8 {
            m.mix_frame(&c);
        }
        assert!(m.output_level_db() < -90.0 && m.output_peak_db() > -40.0);
        m.reset_peaks();
        assert_eq!((m.output_peak_db(), m.stats()[0].1.peak_db), (-100.0, -100.0));
    }

    #[test]
    fn user_volume_and_local_mute_scale_one_speaker_only() {
        let mut a = OpusEncoder::new(true, 64_000).unwrap();
        let mut b = OpusEncoder::new(true, 64_000).unwrap();
        let pa = tone(300.0, 9000.0);
        let pb = tone(700.0, 9000.0);
        let run = |volume_a: f32| {
            let mut a = OpusEncoder::new(true, 64_000).unwrap();
            let mut m = Mixer::default();
            let c = AudioControl::default();
            c.set_user_volume(1, volume_a);
            for s in 0..8 {
                m.push(packet(&mut a, &pa, 1, 10, s));
            }
            (0..6).map(|_| energy(&m.mix_frame(&c))).sum::<f32>()
        };
        let full = run(1.0);
        let half = run(0.5);
        assert!(full > 1.0);
        assert!((half / full - 0.25).abs() < 0.05, "{half} vs {full}");
        assert_eq!(run(0.0), 0.0);

        let mut m = Mixer::default();
        let c = AudioControl::default();
        c.set_user_volume(1, 0.0);
        for s in 0..8 {
            m.push(packet(&mut a, &pa, 1, 10, s));
            m.push(packet(&mut b, &pb, 2, 20, s));
        }
        let e: f32 = (0..6).map(|_| energy(&m.mix_frame(&c))).sum();
        assert!(e > 1.0);
        let st = m.stats();
        assert_eq!(st.len(), 2);
        assert!(st.iter().all(|(_, s)| s.decoded >= 6 && s.level_db > -40.0));
    }

    fn voice_and_stream(c: &AudioControl) -> (f32, f32) {
        let mut va = OpusEncoder::new(true, 64_000).unwrap();
        let mut sa = OpusEncoder::new(true, 64_000).unwrap();
        let voice = tone(300.0, 9000.0);
        let stream = tone(700.0, 9000.0);
        let owner = 42;
        let solo = |track: u64, enc: &mut OpusEncoder, pcm: &[i16]| {
            let mut m = Mixer::default();
            for s in 0..8 {
                m.push(packet(enc, pcm, track, 30, s));
            }
            (0..6).map(|_| energy(&m.mix_frame(c))).sum::<f32>()
        };
        (solo(owner, &mut va, &voice), solo(owner | crate::audio::STREAM_USER_BIT, &mut sa, &stream))
    }

    #[test]
    fn stream_audio_has_its_own_volume_and_mute() {
        let reference = AudioControl::default();
        reference.set_stream_volume(42, 1.0);
        let (voice_full, stream_full) = voice_and_stream(&reference);
        assert!(voice_full > 1.0 && stream_full > 1.0);

        let c = AudioControl::default();
        assert_eq!(c.stream_volume(42), crate::audio::STREAM_DEFAULT_VOLUME);
        let (voice, stream) = voice_and_stream(&c);
        assert!((voice / voice_full - 1.0).abs() < 0.01, "{voice} vs {voice_full}");
        let want = crate::audio::STREAM_DEFAULT_VOLUME.powi(2);
        assert!((stream / stream_full - want).abs() < want * 0.1, "{stream} vs {stream_full}");

        c.set_stream_muted(42, true);
        let (voice, stream) = voice_and_stream(&c);
        assert_eq!(stream, 0.0);
        assert!((voice / voice_full - 1.0).abs() < 0.01);

        c.set_stream_muted(42, false);
        c.set_user_volume(42, 0.0);
        c.set_stream_volume(42, 0.5);
        let (voice, stream) = voice_and_stream(&c);
        assert_eq!(voice, 0.0);
        assert!((stream / stream_full - 0.25).abs() < 0.03, "{stream} vs {stream_full}");

        c.set_stream_volume(42, 9.0);
        assert_eq!(c.stream_volume(42), 2.0);
        c.set_priority_speaker(7, true);
        assert_eq!(c.track_gain(42 | crate::audio::STREAM_USER_BIT), 2.0);
        assert_eq!(c.track_gain(43), crate::audio::PRIORITY_SPEAKER_DUCKING);
        c.set_deafened(true);
        assert_eq!(voice_and_stream(&c), (0.0, 0.0));
    }

    #[test]
    fn muting_a_stream_keeps_the_rest_of_the_mix() {
        let mut v = OpusEncoder::new(true, 64_000).unwrap();
        let mut s = OpusEncoder::new(true, 64_000).unwrap();
        let pv = tone(300.0, 9000.0);
        let ps = tone(900.0, 9000.0);
        let stream = 5 | crate::audio::STREAM_USER_BIT;
        let c = AudioControl::default();
        c.set_stream_volume(5, 1.0);
        let mut m = Mixer::default();
        for q in 0..8 {
            m.push(packet(&mut v, &pv, 5, 1, q));
            m.push(packet(&mut s, &ps, stream, 2, q));
        }
        m.mix_frame(&c);
        let both: f32 = (0..3).map(|_| energy(&m.mix_frame(&c))).sum();
        c.set_stream_muted(5, true);
        let voice_only: f32 = (0..3).map(|_| energy(&m.mix_frame(&c))).sum();
        assert!(both > voice_only * 1.5, "{both} vs {voice_only}");
        assert!(voice_only > 1.0);
        let st = m.stats();
        assert_eq!(st.len(), 2);
        assert!(st.iter().all(|(_, t)| t.level_db > -40.0));
    }

    #[test]
    fn output_volume_scales_and_clips() {
        let mut enc = OpusEncoder::new(true, 64_000).unwrap();
        let pcm = tone(440.0, 30000.0);
        let mut m = Mixer::default();
        let c = AudioControl::default();
        c.set_output_gain(2.0);
        for s in 0..8 {
            m.push(packet(&mut enc, &pcm, 3, 5, s));
        }
        for _ in 0..6 {
            assert!(m.mix_frame(&c).iter().all(|s| (-1.0..=1.0).contains(s)));
        }
        c.set_output_gain(0.0);
        assert!(m.mix_frame(&c).iter().all(|s| *s == 0.0));
    }

    #[test]
    fn single_loss_is_recovered_with_fec_and_bursts_are_concealed() {
        let mut enc = OpusEncoder::new(true, 64_000).unwrap();
        enc.enable_fec(20).unwrap();
        let pcm = tone(440.0, 12000.0);
        let mut m = Mixer::default();
        let c = AudioControl::default();
        for s in 0..20u16 {
            let p = packet(&mut enc, &pcm, 4, 9, s);
            if s != 5 && !(10..13).contains(&s) {
                m.push(p);
            }
            m.mix_frame(&c);
        }
        m.mix_frame(&c);
        let s = m.stats()[0].1;
        assert_eq!(s.received, 16);
        assert_eq!((s.recovered, s.concealed, s.decoded, s.dropped), (2, 2, 16, 0), "{s:?}");
    }

    #[test]
    fn late_and_restarted_streams() {
        let mut enc = OpusEncoder::new(true, 64_000).unwrap();
        let pcm = tone(440.0, 12000.0);
        let mut m = Mixer::default();
        let c = AudioControl::default();
        for s in 100..104 {
            m.push(packet(&mut enc, &pcm, 8, 1, s));
        }
        for _ in 0..4 {
            m.mix_frame(&c);
        }
        m.push(packet(&mut enc, &pcm, 8, 1, 101));
        assert_eq!(m.stats()[0].1.late, 1);
        for s in 0..4 {
            m.push(packet(&mut enc, &pcm, 8, 2, s));
        }
        let st = m.stats()[0].1;
        assert_eq!((st.ssrc, st.resets), (2, 1));
        let e: f32 = (0..3).map(|_| energy(&m.mix_frame(&c))).sum();
        assert!(e > 1.0);
    }
}
