use crate::{Error, Result};

pub const PT_SR: u8 = 200;
pub const PT_RR: u8 = 201;
pub const PT_RTPFB: u8 = 205;
pub const PT_PSFB: u8 = 206;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderReport {
    pub ssrc: u32,
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
}

impl SenderReport {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(28);
        v.push(0x80);
        v.push(PT_SR);
        v.extend_from_slice(&6u16.to_be_bytes());
        v.extend_from_slice(&self.ssrc.to_be_bytes());
        v.extend_from_slice(&self.ntp_timestamp.to_be_bytes());
        v.extend_from_slice(&self.rtp_timestamp.to_be_bytes());
        v.extend_from_slice(&self.packet_count.to_be_bytes());
        v.extend_from_slice(&self.octet_count.to_be_bytes());
        v
    }

    pub fn from_sender(s: &super::RtpSender, now: std::time::SystemTime) -> Self {
        Self {
            ssrc: s.ssrc,
            ntp_timestamp: ntp_time(now),
            rtp_timestamp: s.last_timestamp,
            packet_count: s.packets_sent,
            octet_count: s.octets_sent,
        }
    }
}

pub fn ntp_time(t: std::time::SystemTime) -> u64 {
    const EPOCH_DELTA: u64 = 2_208_988_800;
    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() + EPOCH_DELTA;
    let frac = ((d.subsec_nanos() as u64) << 32) / 1_000_000_000;
    (secs << 32) | frac
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feedback {
    Pli { media_ssrc: u32 },
    Fir { media_ssrc: u32 },
    Nack { media_ssrc: u32, lost: Vec<u16> },
    SenderReport(SenderReport),
    Other(u8),
}

pub fn build_pli(sender_ssrc: u32, media_ssrc: u32) -> Vec<u8> {
    let mut v = vec![0x81, PT_PSFB, 0, 2];
    v.extend_from_slice(&sender_ssrc.to_be_bytes());
    v.extend_from_slice(&media_ssrc.to_be_bytes());
    v
}

pub fn build_nack(sender_ssrc: u32, media_ssrc: u32, lost: &[u16]) -> Vec<u8> {
    let mut seqs = lost.to_vec();
    seqs.sort_unstable_by_key(|s| s.wrapping_sub(lost.first().copied().unwrap_or(0)));
    seqs.dedup();
    let mut fci: Vec<(u16, u16)> = Vec::new();
    for s in seqs {
        if let Some((pid, blp)) = fci.last_mut() {
            let d = s.wrapping_sub(*pid);
            if (1..=16).contains(&d) {
                *blp |= 1 << (d - 1);
                continue;
            }
        }
        fci.push((s, 0));
    }
    let mut v = vec![0x81, PT_RTPFB];
    v.extend_from_slice(&(2 + fci.len() as u16).to_be_bytes());
    v.extend_from_slice(&sender_ssrc.to_be_bytes());
    v.extend_from_slice(&media_ssrc.to_be_bytes());
    for (pid, blp) in fci {
        v.extend_from_slice(&pid.to_be_bytes());
        v.extend_from_slice(&blp.to_be_bytes());
    }
    v
}

pub fn build_fir(sender_ssrc: u32, media_ssrc: u32, seq: u8) -> Vec<u8> {
    let mut v = vec![0x84, PT_PSFB, 0, 4];
    v.extend_from_slice(&sender_ssrc.to_be_bytes());
    v.extend_from_slice(&0u32.to_be_bytes());
    v.extend_from_slice(&media_ssrc.to_be_bytes());
    v.extend_from_slice(&[seq, 0, 0, 0]);
    v
}

pub fn parse(mut p: &[u8]) -> Result<Vec<Feedback>> {
    let mut out = Vec::new();
    while p.len() >= 4 {
        let fmt = p[0] & 0x1f;
        let pt = p[1];
        let len = (u16::from_be_bytes([p[2], p[3]]) as usize + 1) * 4;
        let pkt = p.get(..len).ok_or(Error::Malformed("rtcp length"))?;
        let u32at = |o: usize| -> Result<u32> {
            pkt.get(o..o + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or(Error::Malformed("rtcp field"))
        };
        match (pt, fmt) {
            (PT_PSFB, 1) => out.push(Feedback::Pli { media_ssrc: u32at(8)? }),
            (PT_PSFB, 4) => out.push(Feedback::Fir { media_ssrc: u32at(12)? }),
            (PT_RTPFB, 1) => {
                let mut lost = Vec::new();
                let mut o = 12;
                while o + 4 <= len {
                    let pid = u16::from_be_bytes([pkt[o], pkt[o + 1]]);
                    let blp = u16::from_be_bytes([pkt[o + 2], pkt[o + 3]]);
                    lost.push(pid);
                    for b in 0..16 {
                        if blp & (1 << b) != 0 {
                            lost.push(pid.wrapping_add(b + 1));
                        }
                    }
                    o += 4;
                }
                out.push(Feedback::Nack { media_ssrc: u32at(8)?, lost });
            }
            (PT_SR, _) => out.push(Feedback::SenderReport(SenderReport {
                ssrc: u32at(4)?,
                ntp_timestamp: (u32at(8)? as u64) << 32 | u32at(12)? as u64,
                rtp_timestamp: u32at(16)?,
                packet_count: u32at(20)?,
                octet_count: u32at(24)?,
            })),
            (pt, _) => out.push(Feedback::Other(pt)),
        }
        p = &p[len..];
    }
    Ok(out)
}

pub fn report_blocks(mut p: &[u8]) -> Vec<(u32, u8)> {
    let mut out = Vec::new();
    while p.len() >= 4 {
        let count = (p[0] & 0x1f) as usize;
        let pt = p[1];
        let len = (u16::from_be_bytes([p[2], p[3]]) as usize + 1) * 4;
        let Some(pkt) = p.get(..len) else { break };
        let start = match pt {
            PT_SR => Some(28),
            PT_RR => Some(8),
            _ => None,
        };
        if let Some(start) = start {
            for i in 0..count {
                let o = start + i * 24;
                if let Some(b) = pkt.get(o..o + 5) {
                    out.push((u32::from_be_bytes([b[0], b[1], b[2], b[3]]), b[4]));
                }
            }
        }
        p = &p[len..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiver_report_blocks() {
        let mut v = vec![0x81, PT_RR, 0, 7, 0, 0, 0, 1];
        v.extend_from_slice(&[0, 0, 0, 42, 64, 0, 0, 3]);
        v.extend_from_slice(&[0; 16]);
        assert_eq!(report_blocks(&v), vec![(42, 64)]);
    }

    #[test]
    fn sr_and_pli_roundtrip() {
        let sr = SenderReport { ssrc: 9, ntp_timestamp: 0x1122334455667788, rtp_timestamp: 3, packet_count: 4, octet_count: 5 };
        let mut buf = sr.to_bytes();
        buf.extend_from_slice(&build_pli(1, 77));
        let fb = parse(&buf).unwrap();
        assert_eq!(fb, vec![Feedback::SenderReport(sr), Feedback::Pli { media_ssrc: 77 }]);
    }

    #[test]
    fn nack_build_packs_bitmasks_across_wrap() {
        let pk = build_nack(7, 9, &[65534, 65535, 0, 3, 40]);
        assert_eq!(pk.len(), 12 + 8);
        let fb = parse(&pk).unwrap();
        assert_eq!(fb, vec![Feedback::Nack { media_ssrc: 9, lost: vec![65534, 65535, 0, 3, 40] }]);
        let pk = build_nack(7, 9, &[10, 26, 27]);
        assert_eq!(parse(&pk).unwrap(), vec![Feedback::Nack { media_ssrc: 9, lost: vec![10, 26, 27] }]);
        assert_eq!(&pk[12..16], &[0, 10, 0x80, 0x00]);
    }

    #[test]
    fn fir_roundtrip() {
        let pk = build_fir(1, 55, 3);
        assert_eq!(pk.len(), 20);
        assert_eq!(parse(&pk).unwrap(), vec![Feedback::Fir { media_ssrc: 55 }]);
    }

    #[test]
    fn nack_parse() {
        let mut v = vec![0x81, PT_RTPFB, 0, 3, 0, 0, 0, 1, 0, 0, 0, 5];
        v.extend_from_slice(&[0, 10, 0b0000_0000, 0b0000_0101]);
        assert_eq!(parse(&v).unwrap(), vec![Feedback::Nack { media_ssrc: 5, lost: vec![10, 11, 13] }]);
    }
}
