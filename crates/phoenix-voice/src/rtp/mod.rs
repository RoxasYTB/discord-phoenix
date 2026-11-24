pub mod av1;
pub mod h264;
pub mod jitter;
pub mod rtcp;
pub mod vp8;

use crate::{Error, Result};

pub const DEFAULT_MAX_PAYLOAD: usize = 1100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtpHeader {
    pub marker: bool,
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub extension: Option<(u16, Vec<u8>)>,
}

impl RtpHeader {
    pub fn new(payload_type: u8, sequence: u16, timestamp: u32, ssrc: u32, marker: bool) -> Self {
        Self { marker, payload_type, sequence, timestamp, ssrc, extension: None }
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        let x = if self.extension.is_some() { 0x10 } else { 0 };
        out.push(0x80 | x);
        out.push((self.marker as u8) << 7 | (self.payload_type & 0x7f));
        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&self.timestamp.to_be_bytes());
        out.extend_from_slice(&self.ssrc.to_be_bytes());
        if let Some((profile, body)) = &self.extension {
            out.extend_from_slice(&profile.to_be_bytes());
            let words = body.len().div_ceil(4);
            out.extend_from_slice(&(words as u16).to_be_bytes());
            out.extend_from_slice(body);
            out.resize(out.len() + words * 4 - body.len(), 0);
        }
    }
}

pub fn build_packet(h: &RtpHeader, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(16 + payload.len());
    h.write(&mut v);
    v.extend_from_slice(payload);
    v
}

pub fn parse_packet(p: &[u8]) -> Result<(RtpHeader, &[u8])> {
    if p.len() < 12 || p[0] >> 6 != 2 {
        return Err(Error::Malformed("not rtp v2"));
    }
    let cc = (p[0] & 0x0f) as usize;
    let mut off = 12 + cc * 4;
    let mut end = p.len();
    if p[0] & 0x20 != 0 {
        let pad = *p.last().ok_or(Error::Malformed("pad"))? as usize;
        end = end.checked_sub(pad).ok_or(Error::Malformed("pad"))?;
    }
    let mut extension = None;
    if p[0] & 0x10 != 0 {
        let h = p.get(off..off + 4).ok_or(Error::Malformed("ext hdr"))?;
        let profile = u16::from_be_bytes([h[0], h[1]]);
        let words = u16::from_be_bytes([h[2], h[3]]) as usize;
        let body = p.get(off + 4..off + 4 + words * 4).ok_or(Error::Malformed("ext body"))?;
        extension = Some((profile, body.to_vec()));
        off += 4 + words * 4;
    }
    if off > end {
        return Err(Error::Malformed("rtp payload bounds"));
    }
    Ok((
        RtpHeader {
            marker: p[1] & 0x80 != 0,
            payload_type: p[1] & 0x7f,
            sequence: u16::from_be_bytes([p[2], p[3]]),
            timestamp: u32::from_be_bytes([p[4], p[5], p[6], p[7]]),
            ssrc: u32::from_be_bytes([p[8], p[9], p[10], p[11]]),
            extension,
        },
        &p[off..end],
    ))
}

pub trait Packetizer: Send {
    fn packetize(&mut self, frame: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>>;
}

pub trait Depacketizer: Send {
    fn push(&mut self, payload: &[u8], marker: bool) -> Result<Option<Vec<u8>>>;
}

#[derive(Default)]
pub struct OpusPacketizer;

impl Packetizer for OpusPacketizer {
    fn packetize(&mut self, frame: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>> {
        if frame.len() > max_payload {
            return Err(Error::Malformed("opus frame exceeds MTU"));
        }
        Ok(vec![frame.to_vec()])
    }
}

impl Depacketizer for OpusPacketizer {
    fn push(&mut self, payload: &[u8], _marker: bool) -> Result<Option<Vec<u8>>> {
        Ok(Some(payload.to_vec()))
    }
}

#[derive(Debug, Clone)]
pub struct RtpSender {
    pub ssrc: u32,
    pub payload_type: u8,
    pub sequence: u16,
    pub packets_sent: u32,
    pub octets_sent: u32,
    pub last_timestamp: u32,
}

impl RtpSender {
    pub fn new(ssrc: u32, payload_type: u8) -> Self {
        Self { ssrc, payload_type, sequence: rand::random(), packets_sent: 0, octets_sent: 0, last_timestamp: 0 }
    }

    pub fn wrap(&mut self, payloads: Vec<Vec<u8>>, timestamp: u32) -> Vec<Vec<u8>> {
        let n = payloads.len();
        self.last_timestamp = timestamp;
        payloads
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                let h = RtpHeader::new(self.payload_type, self.sequence, timestamp, self.ssrc, i + 1 == n);
                self.sequence = self.sequence.wrapping_add(1);
                self.packets_sent = self.packets_sent.wrapping_add(1);
                self.octets_sent = self.octets_sent.wrapping_add(p.len() as u32);
                build_packet(&h, &p)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip_with_extension() {
        let mut h = RtpHeader::new(101, 65535, 90000, 0xdeadbeef, true);
        h.extension = Some((0xbede, vec![0x10, 0x01, 0, 0]));
        let p = build_packet(&h, b"abc");
        let (h2, pl) = parse_packet(&p).unwrap();
        assert_eq!(h2, h);
        assert_eq!(pl, b"abc");
    }

    #[test]
    fn sender_sets_marker_on_last() {
        let mut s = RtpSender::new(5, 120);
        s.sequence = 65535;
        let pk = s.wrap(vec![vec![1], vec![2]], 960);
        let (a, _) = parse_packet(&pk[0]).unwrap();
        let (b, _) = parse_packet(&pk[1]).unwrap();
        assert!(!a.marker && b.marker);
        assert_eq!((a.sequence, b.sequence), (65535, 0));
    }
}
