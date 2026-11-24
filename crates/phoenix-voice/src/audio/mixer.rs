use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::AudioControl;
use super::dsp::mix_into;
use crate::FRAME_SAMPLES;
use crate::connection::AudioIn;
use crate::media::AudioDecoder;
use crate::media::opus::OpusDecoder;
use crate::rtp::jitter::{JitterBuffer, JitterOutput, JitterPacket};

const PREBUFFER: usize = 2;
const MAX_QUEUE: usize = 10;
const MAX_PLC: u32 = 3;
const IDLE_DROP: Duration = Duration::from_secs(10);

struct Track {
    jitter: JitterBuffer,
    decoder: OpusDecoder,
    playing: bool,
    plc_run: u32,
    last_push: Instant,
}

impl Track {
    fn new() -> Option<Self> {
        Some(Self {
            jitter: JitterBuffer::new(4, 64),
            decoder: OpusDecoder::new(true).ok()?,
            playing: false,
            plc_run: 0,
            last_push: Instant::now(),
        })
    }

    fn next_frame(&mut self) -> Option<Vec<i16>> {
        if !self.playing {
            if self.jitter.len() < PREBUFFER {
                return None;
            }
            self.playing = true;
        }
        while self.jitter.len() > MAX_QUEUE {
            self.jitter.pop();
        }
        match self.jitter.pop() {
            Some(JitterOutput::Packet(p)) => {
                self.plc_run = 0;
                self.decoder.decode(Some(&p.payload)).ok()
            }
            Some(JitterOutput::Lost { .. }) => {
                self.plc_run = 0;
                self.decoder.decode(None).ok()
            }
            None => {
                self.plc_run += 1;
                if self.plc_run > MAX_PLC {
                    self.playing = false;
                    return None;
                }
                self.decoder.decode(None).ok()
            }
        }
    }
}

#[derive(Default)]
pub struct Mixer {
    tracks: HashMap<u64, Track>,
}

impl Mixer {
    pub fn push(&mut self, a: AudioIn) {
        let t = match self.tracks.entry(a.user_id) {
            std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => match Track::new() {
                Some(t) => v.insert(t),
                None => return,
            },
        };
        t.last_push = Instant::now();
        t.jitter.push(JitterPacket { sequence: a.sequence, timestamp: a.timestamp, marker: false, payload: a.opus });
    }

    pub fn remove_user(&mut self, user: u64) {
        self.tracks.remove(&user);
    }

    pub fn mix_frame(&mut self, control: &AudioControl) -> Vec<f32> {
        let mut acc = vec![0f32; FRAME_SAMPLES * 2];
        let now = Instant::now();
        self.tracks.retain(|_, t| now.duration_since(t.last_push) < IDLE_DROP);
        let deaf = control.deafened();
        for (uid, t) in self.tracks.iter_mut() {
            if let Some(pcm) = t.next_frame()
                && !deaf
            {
                mix_into(&mut acc, &pcm, control.user_volume(*uid));
            }
        }
        let g = control.output_gain();
        if g != 1.0 {
            acc.iter_mut().for_each(|s| *s *= g);
        }
        acc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::AudioEncoder;
    use crate::media::opus::OpusEncoder;

    #[test]
    fn mixes_after_prebuffer_and_respects_deafen() {
        let mut enc = OpusEncoder::new(true, 64_000).unwrap();
        let pcm: Vec<i16> = (0..FRAME_SAMPLES * 2).map(|i| ((i as f32 * 0.05).sin() * 12000.0) as i16).collect();
        let c = AudioControl::default();
        let mut m = Mixer::default();
        let push = |m: &mut Mixer, enc: &mut OpusEncoder, seq: u16| {
            m.push(AudioIn { user_id: 7, ssrc: 1, sequence: seq, timestamp: seq as u32 * 960, opus: enc.encode(&pcm).unwrap() });
        };
        push(&mut m, &mut enc, 0);
        assert!(m.mix_frame(&c).iter().all(|s| *s == 0.0));
        for s in 1..6 {
            push(&mut m, &mut enc, s);
        }
        let mut energy = 0.0;
        for _ in 0..4 {
            energy += m.mix_frame(&c).iter().map(|s| s * s).sum::<f32>();
        }
        assert!(energy > 1.0);
        c.set_deafened(true);
        assert!(m.mix_frame(&c).iter().all(|s| *s == 0.0));
    }
}
