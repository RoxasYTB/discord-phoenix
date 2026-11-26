use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use crate::codec::Codec;
use crate::gateway::protocol::VideoPayload;
use crate::rtp::{self, Depacketizer, RtpHeader};

pub const NACK_INTERVAL: Duration = Duration::from_millis(100);
pub const NACK_MAX_RETRIES: u8 = 10;
pub const MAX_NACK_LIST: u64 = 1000;
pub const KEYFRAME_INTERVAL: Duration = Duration::from_millis(200);
pub const GAP_TIMEOUT: Duration = Duration::from_millis(400);
pub const MAX_BUFFERED: usize = 2048;
pub const LIVE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layer {
    pub ssrc: u32,
    pub rtx_ssrc: u32,
    pub quality: u32,
}

pub fn layers(v: &VideoPayload) -> Vec<Layer> {
    let mut out: Vec<Layer> = v
        .streams
        .iter()
        .filter(|s| s.active && s.ssrc != 0)
        .map(|s| Layer { ssrc: s.ssrc, rtx_ssrc: s.rtx_ssrc, quality: if s.quality == 0 { 100 } else { s.quality } })
        .collect();
    if out.is_empty() && v.video_ssrc != 0 {
        out.push(Layer { ssrc: v.video_ssrc, rtx_ssrc: v.rtx_ssrc, quality: 100 });
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedFrame {
    pub user_id: u64,
    pub ssrc: u32,
    pub codec: Codec,
    pub timestamp: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Nack { media_ssrc: u32, lost: Vec<u16> },
    KeyFrame { media_ssrc: u32 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RxStats {
    pub packets: u64,
    pub rtx: u64,
    pub frames: u64,
    pub dropped_frames: u64,
    pub nacks: u64,
    pub keyframe_requests: u64,
}

struct Packet {
    payload_type: u8,
    marker: bool,
    timestamp: u32,
    payload: Vec<u8>,
}

struct Missing {
    since: Instant,
    sent: Option<Instant>,
    tries: u8,
}

struct Track {
    user: u64,
    packets: BTreeMap<u64, Packet>,
    last: Option<u64>,
    next: Option<u64>,
    missing: BTreeMap<u64, Missing>,
    waiting_key: bool,
    last_key_request: Option<Instant>,
    heard: Option<Instant>,
}

impl Track {
    fn new(user: u64) -> Self {
        Self { user, packets: BTreeMap::new(), last: None, next: None, missing: BTreeMap::new(), waiting_key: true, last_key_request: None, heard: None }
    }

    fn unwrap_seq(&self, seq: u16) -> u64 {
        let Some(last) = self.last else { return (1u64 << 32) | seq as u64 };
        let base = last & !0xffff;
        let mut best = base | seq as u64;
        for cand in [best.wrapping_add(1 << 16), best.wrapping_sub(1 << 16)] {
            if cand.abs_diff(last) < best.abs_diff(last) {
                best = cand;
            }
        }
        best
    }

    fn reset(&mut self) {
        self.packets.clear();
        self.missing.clear();
        self.next = None;
        self.waiting_key = true;
    }

    fn drop_through(&mut self, ext: u64) {
        self.packets.retain(|s, _| *s > ext);
        self.missing.retain(|s, _| *s > ext);
        self.next = Some(ext + 1);
    }
}

#[derive(Default)]
pub struct VideoReceiver {
    tracks: HashMap<u32, Track>,
    rtx: HashMap<u32, u32>,
    users: HashMap<u64, Vec<Layer>>,
    current: HashMap<u64, u32>,
    stats: RxStats,
}

impl VideoReceiver {
    pub fn stats(&self) -> RxStats {
        self.stats
    }

    pub fn user_for(&self, ssrc: u32) -> Option<u64> {
        let media = self.rtx.get(&ssrc).copied().unwrap_or(ssrc);
        self.tracks.get(&media).map(|t| t.user)
    }

    pub fn layers(&self, user: u64) -> &[Layer] {
        self.users.get(&user).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn set_layers(&mut self, user: u64, layers: Vec<Layer>) {
        let keep: Vec<u32> = layers.iter().map(|l| l.ssrc).collect();
        self.tracks.retain(|ssrc, t| t.user != user || keep.contains(ssrc));
        self.rtx.retain(|_, media| self.tracks.contains_key(media) || keep.contains(media));
        for l in &layers {
            self.tracks.entry(l.ssrc).or_insert_with(|| Track::new(user)).user = user;
            if l.rtx_ssrc != 0 {
                self.rtx.insert(l.rtx_ssrc, l.ssrc);
            }
        }
        if layers.is_empty() {
            self.users.remove(&user);
            self.current.remove(&user);
        } else {
            if self.current.get(&user).is_some_and(|c| !keep.contains(c)) {
                self.current.remove(&user);
            }
            self.users.insert(user, layers);
        }
    }

    pub fn remove_user(&mut self, user: u64) {
        self.set_layers(user, Vec::new());
    }

    pub fn push(&mut self, h: &RtpHeader, payload: &[u8], now: Instant) -> Vec<EncodedFrame> {
        let (ssrc, seq, pt, payload) = match self.rtx.get(&h.ssrc) {
            Some(&media) => {
                if payload.len() <= 2 {
                    return Vec::new();
                }
                self.stats.rtx += 1;
                (media, u16::from_be_bytes([payload[0], payload[1]]), h.payload_type.wrapping_sub(1), &payload[2..])
            }
            None => (h.ssrc, h.sequence, h.payload_type, payload),
        };
        let Some(t) = self.tracks.get_mut(&ssrc) else { return Vec::new() };
        if !matches!(Codec::from_payload_type(pt), Some((c, false)) if c.is_video()) {
            return Vec::new();
        }
        self.stats.packets += 1;
        t.heard = Some(now);
        let ext = t.unwrap_seq(seq);
        match t.last {
            None => t.last = Some(ext),
            Some(last) if ext > last => {
                if ext - last > MAX_NACK_LIST {
                    t.reset();
                } else {
                    for s in last + 1..ext {
                        t.missing.entry(s).or_insert(Missing { since: now, sent: None, tries: 0 });
                    }
                }
                t.last = Some(ext);
            }
            Some(last) if last - ext > MAX_NACK_LIST => {
                t.reset();
                t.last = Some(ext);
            }
            Some(_) => {}
        }
        if t.next.is_some_and(|n| ext < n) || t.packets.contains_key(&ext) {
            t.missing.remove(&ext);
            return Vec::new();
        }
        t.missing.remove(&ext);
        t.packets.insert(ext, Packet { payload_type: pt, marker: h.marker, timestamp: h.timestamp, payload: payload.to_vec() });
        if t.packets.len() > MAX_BUFFERED {
            t.reset();
            return Vec::new();
        }
        self.assemble(ssrc)
    }

    fn assemble(&mut self, ssrc: u32) -> Vec<EncodedFrame> {
        let mut out = Vec::new();
        let Some(t) = self.tracks.get_mut(&ssrc) else { return out };
        loop {
            let start = match t.next {
                Some(n) => n,
                None => {
                    let Some((&m, _)) = t.packets.iter().find(|(_, p)| p.marker) else { break };
                    t.drop_through(m);
                    continue;
                }
            };
            let mut end = None;
            let mut s = start;
            while let Some(p) = t.packets.get(&s) {
                if p.marker {
                    end = Some(s);
                    break;
                }
                s += 1;
            }
            let Some(end) = end else { break };
            let first = &t.packets[&start];
            let (pt, ts) = (first.payload_type, first.timestamp);
            let codec = Codec::from_payload_type(pt).map(|c| c.0);
            let whole = (start..=end).all(|s| t.packets.get(&s).is_some_and(|p| p.timestamp == ts && p.payload_type == pt));
            let data = match (codec, whole) {
                (Some(c), true) => depacketize(c, (start..=end).filter_map(|s| t.packets.get(&s))),
                _ => None,
            };
            t.drop_through(end);
            match (codec, data) {
                (Some(codec), Some(data)) if !data.is_empty() => {
                    self.stats.frames += 1;
                    out.push(EncodedFrame { user_id: t.user, ssrc, codec, timestamp: ts, data });
                }
                _ => {
                    self.stats.dropped_frames += 1;
                    t.waiting_key = true;
                }
            }
        }
        out
    }

    pub fn accept(&mut self, f: &EncodedFrame, keyframe: bool) -> bool {
        let Some(t) = self.tracks.get_mut(&f.ssrc) else { return false };
        if t.waiting_key && !keyframe {
            self.stats.dropped_frames += 1;
            return false;
        }
        t.waiting_key = false;
        match self.current.get(&f.user_id) {
            Some(&c) if c == f.ssrc => true,
            Some(_) if !keyframe => false,
            _ => {
                self.current.insert(f.user_id, f.ssrc);
                true
            }
        }
    }

    pub fn request_keyframe(&mut self, user: u64) {
        let ssrc = self.current.get(&user).copied();
        for (s, t) in self.tracks.iter_mut() {
            if t.user == user && ssrc.is_none_or(|c| c == *s) {
                t.waiting_key = true;
            }
        }
    }

    pub fn poll(&mut self, now: Instant) -> (Vec<Request>, Vec<EncodedFrame>) {
        let mut out = Vec::new();
        let mut give_up = Vec::new();
        for (&ssrc, t) in self.tracks.iter_mut() {
            let mut lost = Vec::new();
            t.missing.retain(|&s, m| {
                if m.tries >= NACK_MAX_RETRIES {
                    return m.sent.is_none_or(|at| now.duration_since(at) < NACK_INTERVAL);
                }
                if m.sent.is_none_or(|at| now.duration_since(at) >= NACK_INTERVAL) {
                    m.sent = Some(now);
                    m.tries += 1;
                    lost.push(s as u16);
                }
                true
            });
            if !lost.is_empty() {
                self.stats.nacks += 1;
                out.push(Request::Nack { media_ssrc: ssrc, lost });
            }
            if t.next.is_some() && t.missing.values().next().is_some_and(|m| now.duration_since(m.since) >= GAP_TIMEOUT) {
                give_up.push(ssrc);
            }
            let live = t.heard.is_some_and(|at| now.duration_since(at) < LIVE_WINDOW);
            if live && t.waiting_key && t.last_key_request.is_none_or(|at| now.duration_since(at) >= KEYFRAME_INTERVAL) {
                t.last_key_request = Some(now);
                self.stats.keyframe_requests += 1;
                out.push(Request::KeyFrame { media_ssrc: ssrc });
            }
        }
        let mut frames = Vec::new();
        for ssrc in give_up {
            if let Some(t) = self.tracks.get_mut(&ssrc) {
                let next = t.next.unwrap_or(0);
                match t.packets.range(next..).find(|(_, p)| p.marker).map(|(&s, _)| s) {
                    Some(m) => t.drop_through(m),
                    None => {
                        let upto = t.missing.keys().next_back().copied().unwrap_or(next);
                        t.packets.retain(|s, _| *s > upto);
                        t.missing.clear();
                        t.next = None;
                    }
                }
                t.waiting_key = true;
                self.stats.dropped_frames += 1;
            }
            frames.extend(self.assemble(ssrc));
        }
        (out, frames)
    }
}

fn depacketize<'a>(codec: Codec, packets: impl Iterator<Item = &'a Packet>) -> Option<Vec<u8>> {
    let mut d: Box<dyn Depacketizer> = match codec {
        Codec::H264 => Box::new(rtp::h264::H264Depacketizer::default()),
        Codec::Vp8 => Box::new(rtp::vp8::Vp8Depacketizer::default()),
        Codec::Av1 => Box::new(rtp::av1::Av1Depacketizer::default()),
        _ => return None,
    };
    let mut packets = packets.peekable();
    while let Some(p) = packets.next() {
        let last = packets.peek().is_none();
        match d.push(&p.payload, last) {
            Ok(Some(f)) if last => return Some(f),
            Ok(_) if !last => {}
            _ => return None,
        }
    }
    None
}

pub fn rtp_packets(codec: Codec, frames: &[Vec<u8>], ssrc: u32, first_seq: u16, max_payload: usize) -> Option<Vec<(RtpHeader, Vec<u8>)>> {
    use crate::rtp::Packetizer;
    let mut out = Vec::new();
    let mut seq = first_seq;
    for (i, f) in frames.iter().enumerate() {
        let payloads = match codec {
            Codec::H264 => rtp::h264::H264Packetizer::new().packetize(f, max_payload),
            Codec::Vp8 => rtp::vp8::Vp8Packetizer::default().packetize(f, max_payload),
            Codec::Av1 => rtp::av1::Av1Packetizer.packetize(f, max_payload),
            _ => return None,
        }
        .ok()?;
        let n = payloads.len();
        for (k, p) in payloads.into_iter().enumerate() {
            out.push((RtpHeader::new(codec.payload_type(), seq, (i as u32 + 1) * 3000, ssrc, k + 1 == n), p));
            seq = seq.wrapping_add(1);
        }
    }
    Some(out)
}

pub fn is_keyframe(codec: Codec, frame: &[u8]) -> bool {
    match codec {
        Codec::H264 => rtp::h264::is_keyframe(frame),
        Codec::Vp8 => rtp::vp8::is_keyframe(frame),
        Codec::Av1 => rtp::av1::is_keyframe(frame),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::protocol::StreamDescriptor;
    use crate::rtp::Packetizer;

    const SSRC: u32 = 500;
    const RTX: u32 = 501;

    fn rx() -> VideoReceiver {
        let mut r = VideoReceiver::default();
        r.set_layers(7, vec![Layer { ssrc: SSRC, rtx_ssrc: RTX, quality: 100 }]);
        r
    }

    fn vp8_frame(key: bool, len: usize, fill: u8) -> Vec<u8> {
        let mut f = vec![if key { 0x10 } else { 0x11 }];
        f.extend(std::iter::repeat_n(fill, len));
        f
    }

    fn packets(codec: Codec, frame: &[u8], seq: u16, ts: u32, ssrc: u32) -> Vec<(RtpHeader, Vec<u8>)> {
        let pl = match codec {
            Codec::Vp8 => rtp::vp8::Vp8Packetizer::default().packetize(frame, 300),
            Codec::H264 => rtp::h264::H264Packetizer::new().packetize(frame, 300),
            Codec::Av1 => rtp::av1::Av1Packetizer.packetize(frame, 300),
            _ => unreachable!(),
        }
        .unwrap();
        let n = pl.len();
        pl.into_iter()
            .enumerate()
            .map(|(i, p)| (RtpHeader::new(codec.payload_type(), seq.wrapping_add(i as u16), ts, ssrc, i + 1 == n), p))
            .collect()
    }

    fn feed(r: &mut VideoReceiver, pk: &[(RtpHeader, Vec<u8>)], now: Instant) -> Vec<EncodedFrame> {
        pk.iter().flat_map(|(h, p)| r.push(h, p, now)).collect()
    }

    #[test]
    fn layers_from_op12() {
        let v = VideoPayload {
            user_id: Some("7".into()),
            audio_ssrc: 1,
            video_ssrc: 2,
            rtx_ssrc: 3,
            streams: vec![
                StreamDescriptor { kind: "video".into(), rid: "50".into(), ssrc: 4, rtx_ssrc: 5, active: true, quality: 50, ..Default::default() },
                StreamDescriptor { kind: "video".into(), rid: "100".into(), ssrc: 2, rtx_ssrc: 3, active: true, quality: 100, ..Default::default() },
                StreamDescriptor { kind: "video".into(), rid: "x".into(), ssrc: 9, rtx_ssrc: 10, active: false, quality: 100, ..Default::default() },
            ],
        };
        assert_eq!(layers(&v), vec![Layer { ssrc: 4, rtx_ssrc: 5, quality: 50 }, Layer { ssrc: 2, rtx_ssrc: 3, quality: 100 }]);
        let bare = VideoPayload { streams: vec![], ..v.clone() };
        assert_eq!(layers(&bare), vec![Layer { ssrc: 2, rtx_ssrc: 3, quality: 100 }]);
        let off = VideoPayload { video_ssrc: 0, streams: vec![], ..v };
        assert!(layers(&off).is_empty());
    }

    #[test]
    fn in_order_frames_wait_for_a_keyframe_then_play() {
        let mut r = rx();
        let now = Instant::now();
        let delta = vp8_frame(false, 700, 3);
        let key = vp8_frame(true, 900, 4);
        let mut seq = 65530u16;
        let mut got = Vec::new();
        for (i, f) in [&delta, &key, &delta].into_iter().enumerate() {
            let pk = packets(Codec::Vp8, f, seq, i as u32 * 3000, SSRC);
            seq = seq.wrapping_add(pk.len() as u16);
            got.extend(feed(&mut r, &pk, now));
        }
        assert_eq!(got.len(), 2, "the first frame is only used to find a frame boundary");
        assert_eq!(got[0].data, key);
        assert_eq!(got[1].data, delta);
        assert!(got.iter().all(|f| f.codec == Codec::Vp8 && f.user_id == 7));
        let (req, _) = r.poll(now);
        assert_eq!(req, vec![Request::KeyFrame { media_ssrc: SSRC }]);
        assert!(r.accept(&got[0], true));
        assert!(r.accept(&got[1], false));
        assert!(r.poll(now + KEYFRAME_INTERVAL).0.is_empty());
    }

    #[test]
    fn delta_frames_are_held_back_until_a_keyframe() {
        let mut r = rx();
        let f = EncodedFrame { user_id: 7, ssrc: SSRC, codec: Codec::Vp8, timestamp: 0, data: vp8_frame(false, 4, 1) };
        assert!(!r.accept(&f, false));
        assert!(r.accept(&f, true));
        assert!(r.accept(&f, false));
        r.request_keyframe(7);
        assert!(!r.accept(&f, false));
    }

    #[test]
    fn reordered_packets_assemble_and_a_gap_is_nacked() {
        let mut r = rx();
        let t0 = Instant::now();
        let boundary = packets(Codec::Vp8, &vp8_frame(true, 10, 0), 100, 0, SSRC);
        feed(&mut r, &boundary, t0);
        let key = vp8_frame(true, 1200, 9);
        let mut pk = packets(Codec::Vp8, &key, 101, 3000, SSRC);
        assert!(pk.len() >= 4);
        let late = pk.remove(1);
        assert!(feed(&mut r, &pk, t0).is_empty());
        let (req, _) = r.poll(t0);
        assert!(req.contains(&Request::Nack { media_ssrc: SSRC, lost: vec![late.0.sequence] }));
        assert!(r.poll(t0 + Duration::from_millis(10)).0.iter().all(|q| !matches!(q, Request::Nack { .. })));
        let resent = r.poll(t0 + NACK_INTERVAL).0;
        assert!(resent.contains(&Request::Nack { media_ssrc: SSRC, lost: vec![late.0.sequence] }));
        let got = r.push(&late.0, &late.1, t0 + NACK_INTERVAL);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].data, key);
        assert!(r.poll(t0 + NACK_INTERVAL * 3).0.iter().all(|q| !matches!(q, Request::Nack { .. })));
    }

    #[test]
    fn rtx_retransmission_fills_the_gap() {
        let mut r = rx();
        let t0 = Instant::now();
        feed(&mut r, &packets(Codec::Vp8, &vp8_frame(true, 10, 0), 9, 0, SSRC), t0);
        let key = vp8_frame(true, 800, 5);
        let mut pk = packets(Codec::Vp8, &key, 10, 90, SSRC);
        let (h, p) = pk.remove(0);
        assert!(feed(&mut r, &pk, t0).is_empty());
        let mut rtx_payload = h.sequence.to_be_bytes().to_vec();
        rtx_payload.extend_from_slice(&p);
        let rh = RtpHeader::new(Codec::Vp8.payload_type() + 1, 4000, h.timestamp, RTX, h.marker);
        assert_eq!(r.user_for(RTX), Some(7));
        let got = r.push(&rh, &rtx_payload, t0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].data, key);
        assert_eq!(r.stats().rtx, 1);
        assert!(r.push(&RtpHeader::new(108, 4001, 0, RTX, false), &[0, 1], t0).is_empty());
    }

    #[test]
    fn an_unrecoverable_loss_skips_to_the_next_frame_and_asks_for_a_keyframe() {
        let mut r = rx();
        let t0 = Instant::now();
        feed(&mut r, &packets(Codec::Vp8, &vp8_frame(true, 10, 0), 1, 0, SSRC), t0);
        let key = packets(Codec::Vp8, &vp8_frame(true, 50, 1), 2, 100, SSRC);
        let f = feed(&mut r, &key, t0);
        assert!(r.accept(&f[0], true));
        let mut lossy = packets(Codec::Vp8, &vp8_frame(false, 900, 2), 3, 200, SSRC);
        lossy.remove(1);
        let after = packets(Codec::Vp8, &vp8_frame(false, 20, 3), 3 + lossy.len() as u16 + 1, 300, SSRC);
        assert!(feed(&mut r, &lossy, t0).is_empty());
        assert!(feed(&mut r, &after, t0).is_empty());
        let (_, frames) = r.poll(t0 + GAP_TIMEOUT / 2);
        assert!(frames.is_empty());
        let (req, frames) = r.poll(t0 + GAP_TIMEOUT);
        assert_eq!(frames.len(), 1);
        assert!(!r.accept(&frames[0], false));
        assert!(r.poll(t0 + GAP_TIMEOUT + KEYFRAME_INTERVAL).0.contains(&Request::KeyFrame { media_ssrc: SSRC }) || req.contains(&Request::KeyFrame { media_ssrc: SSRC }));
        assert!(r.stats().dropped_frames >= 2);
    }

    #[test]
    fn nacks_repeat_until_the_gap_is_given_up() {
        let mut r = rx();
        let t0 = Instant::now();
        feed(&mut r, &packets(Codec::Vp8, &vp8_frame(true, 10, 0), 1, 0, SSRC), t0);
        r.push(&RtpHeader::new(107, 5, 9, SSRC, false), &[0x90, 0x80, 0x80, 0x01, 0x10, 0], t0);
        let mut nacks = 0;
        for i in 0..20u32 {
            nacks += r.poll(t0 + NACK_INTERVAL * i).0.iter().filter(|q| matches!(q, Request::Nack { .. })).count();
        }
        let expected = (GAP_TIMEOUT.as_millis() / NACK_INTERVAL.as_millis()) as usize + 1;
        assert_eq!(nacks, expected.min(NACK_MAX_RETRIES as usize));
    }

    #[test]
    fn h264_and_av1_frames_come_out_whole() {
        let mut r = rx();
        let t0 = Instant::now();
        let mut idr = vec![0, 0, 0, 1, 0x67, 0x42, 0xe0, 0x1f, 0, 0, 0, 1, 0x68, 0xce, 0x3c, 0x80, 0, 0, 0, 1, 0x65];
        idr.extend((0..2000).map(|i| (i % 250) as u8 | 1));
        feed(&mut r, &packets(Codec::H264, &idr, 10, 0, SSRC), t0);
        let n = packets(Codec::H264, &idr, 10, 0, SSRC).len() as u16;
        let pk = packets(Codec::H264, &idr, 10 + n, 3000, SSRC);
        let next_seq = 10 + n + pk.len() as u16;
        let got = feed(&mut r, &pk, t0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].codec, Codec::H264);
        assert!(is_keyframe(Codec::H264, &got[0].data));
        assert!(got[0].data.ends_with(&idr[idr.len() - 100..]));

        let mut av1 = vec![(1 << 3) | 2, 4, 0, 0, 0, 0x42, (6 << 3) | 2];
        let body: Vec<u8> = (0..1500u32).map(|i| (i * 3) as u8).collect();
        crate::leb128::write(body.len() as u64, &mut av1);
        av1.extend_from_slice(&body);
        let got = feed(&mut r, &packets(Codec::Av1, &av1, next_seq, 6000, SSRC), t0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].codec, Codec::Av1);
        assert_eq!(got[0].data, av1);
        assert!(is_keyframe(Codec::Av1, &got[0].data));
    }

    #[test]
    fn simulcast_switch_waits_for_a_keyframe_on_the_new_layer() {
        let mut r = VideoReceiver::default();
        r.set_layers(7, vec![Layer { ssrc: 1, rtx_ssrc: 0, quality: 50 }, Layer { ssrc: 2, rtx_ssrc: 0, quality: 100 }]);
        let f = |ssrc| EncodedFrame { user_id: 7, ssrc, codec: Codec::Vp8, timestamp: 0, data: vec![0x11] };
        assert!(r.accept(&f(1), true));
        assert!(r.accept(&f(1), false));
        assert!(!r.accept(&f(2), false));
        assert!(r.accept(&f(2), true));
        assert!(!r.accept(&f(1), false));
        r.set_layers(7, vec![Layer { ssrc: 1, rtx_ssrc: 0, quality: 50 }]);
        assert_eq!(r.user_for(2), None);
        assert_eq!(r.layers(7).len(), 1);
        r.remove_user(7);
        assert_eq!(r.user_for(1), None);
        assert!(r.layers(7).is_empty());
    }
}
