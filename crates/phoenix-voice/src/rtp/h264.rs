use super::{Depacketizer, Packetizer};
use crate::{Error, Result};

pub const NAL_SLICE: u8 = 1;
pub const NAL_IDR: u8 = 5;
pub const NAL_SEI: u8 = 6;
pub const NAL_SPS: u8 = 7;
pub const NAL_PPS: u8 = 8;
pub const NAL_AUD: u8 = 9;
const STAP_A: u8 = 24;
const FU_A: u8 = 28;

pub fn split_annex_b(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (k, &s) in starts.iter().enumerate() {
        let mut e = starts.get(k + 1).map(|n| n - 3).unwrap_or(data.len());
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

pub fn nal_type(nal: &[u8]) -> u8 {
    nal.first().map(|b| b & 0x1f).unwrap_or(0)
}

pub fn is_keyframe(au: &[u8]) -> bool {
    split_annex_b(au).iter().any(|n| nal_type(n) == NAL_IDR)
}

#[derive(Default)]
pub struct H264Packetizer {
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl H264Packetizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare_nals<'a>(&mut self, au: &'a [u8]) -> Vec<std::borrow::Cow<'a, [u8]>> {
        use std::borrow::Cow;
        let nals: Vec<&[u8]> = split_annex_b(au).into_iter().filter(|n| nal_type(n) != NAL_AUD).collect();
        let mut has_sps = false;
        let mut has_pps = false;
        for n in &nals {
            match nal_type(n) {
                NAL_SPS => {
                    has_sps = true;
                    self.sps = Some(n.to_vec());
                }
                NAL_PPS => {
                    has_pps = true;
                    self.pps = Some(n.to_vec());
                }
                _ => {}
            }
        }
        let idr = nals.iter().any(|n| nal_type(n) == NAL_IDR);
        let mut out: Vec<Cow<'a, [u8]>> = Vec::with_capacity(nals.len() + 2);
        if idr {
            if !has_sps && let Some(s) = &self.sps {
                out.push(Cow::Owned(s.clone()));
            }
            if !has_pps && let Some(p) = &self.pps {
                out.push(Cow::Owned(p.clone()));
            }
        }
        out.extend(nals.into_iter().map(Cow::Borrowed));
        out
    }
}

impl H264Packetizer {
    pub fn prepare_access_unit(&mut self, au: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(au.len() + 64);
        for n in self.prepare_nals(au) {
            out.extend_from_slice(&START);
            out.extend_from_slice(&n);
        }
        out
    }

    pub fn packetize_prepared(&self, au: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>> {
        let nals: Vec<std::borrow::Cow<'_, [u8]>> =
            split_annex_b(au).into_iter().map(std::borrow::Cow::Borrowed).collect();
        packetize_nals(&nals, max_payload)
    }
}

impl Packetizer for H264Packetizer {
    fn packetize(&mut self, au: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>> {
        let nals = self.prepare_nals(au);
        packetize_nals(&nals, max_payload)
    }
}

fn packetize_nals(nals: &[std::borrow::Cow<'_, [u8]>], max_payload: usize) -> Result<Vec<Vec<u8>>> {
    {
        if max_payload < 16 {
            return Err(Error::Malformed("mtu too small"));
        }
        if nals.is_empty() {
            return Err(Error::Malformed("empty access unit"));
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < nals.len() {
            let n = &nals[i];
            if n.len() > max_payload {
                let hdr = n[0];
                let ind = (hdr & 0xe0) | FU_A;
                let body = &n[1..];
                let chunk = max_payload - 2;
                let count = body.len().div_ceil(chunk);
                for (k, c) in body.chunks(chunk).enumerate() {
                    let mut fu = hdr & 0x1f;
                    if k == 0 {
                        fu |= 0x80;
                    }
                    if k + 1 == count {
                        fu |= 0x40;
                    }
                    let mut p = Vec::with_capacity(c.len() + 2);
                    p.push(ind);
                    p.push(fu);
                    p.extend_from_slice(c);
                    out.push(p);
                }
                i += 1;
                continue;
            }
            let mut j = i;
            let mut size = 1;
            while j < nals.len() && size + 2 + nals[j].len() <= max_payload {
                size += 2 + nals[j].len();
                j += 1;
            }
            if j - i >= 2 {
                let nri = nals[i..j].iter().map(|n| n[0] & 0x60).max().unwrap_or(0);
                let mut p = Vec::with_capacity(size);
                p.push(nri | STAP_A);
                for n in &nals[i..j] {
                    p.extend_from_slice(&(n.len() as u16).to_be_bytes());
                    p.extend_from_slice(n);
                }
                out.push(p);
                i = j;
            } else {
                out.push(n.to_vec());
                i += 1;
            }
        }
        Ok(out)
    }
}

#[derive(Default)]
pub struct H264Depacketizer {
    au: Vec<u8>,
    in_fu: bool,
}

const START: [u8; 4] = [0, 0, 0, 1];

impl Depacketizer for H264Depacketizer {
    fn push(&mut self, p: &[u8], marker: bool) -> Result<Option<Vec<u8>>> {
        let first = *p.first().ok_or(Error::Malformed("empty h264 payload"))?;
        match first & 0x1f {
            1..=23 => {
                self.au.extend_from_slice(&START);
                self.au.extend_from_slice(p);
            }
            STAP_A => {
                let mut o = 1;
                while o + 2 <= p.len() {
                    let l = u16::from_be_bytes([p[o], p[o + 1]]) as usize;
                    let n = p.get(o + 2..o + 2 + l).ok_or(Error::Malformed("stap-a"))?;
                    self.au.extend_from_slice(&START);
                    self.au.extend_from_slice(n);
                    o += 2 + l;
                }
            }
            FU_A => {
                let fu = *p.get(1).ok_or(Error::Malformed("fu-a"))?;
                if fu & 0x80 != 0 {
                    self.au.extend_from_slice(&START);
                    self.au.push((first & 0xe0) | (fu & 0x1f));
                    self.in_fu = true;
                } else if !self.in_fu {
                    return Err(Error::Malformed("fu-a continuation without start"));
                }
                self.au.extend_from_slice(&p[2..]);
                if fu & 0x40 != 0 {
                    self.in_fu = false;
                }
            }
            _ => return Err(Error::Unsupported("h264 nal type".into())),
        }
        if marker {
            self.in_fu = false;
            return Ok(Some(std::mem::take(&mut self.au)));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn au(nals: &[Vec<u8>]) -> Vec<u8> {
        let mut v = Vec::new();
        for n in nals {
            v.extend_from_slice(&START);
            v.extend_from_slice(n);
        }
        v
    }

    fn roundtrip(input: &[u8], pk: &mut H264Packetizer, mtu: usize) -> Vec<u8> {
        let pl = pk.packetize(input, mtu).unwrap();
        let mut d = H264Depacketizer::default();
        let mut res = None;
        for (i, p) in pl.iter().enumerate() {
            assert!(p.len() <= mtu);
            res = d.push(p, i + 1 == pl.len()).unwrap();
        }
        res.unwrap()
    }

    #[test]
    fn stap_and_fu_roundtrip() {
        let sps = vec![0x67, 0x42, 0xe0, 0x1f, 0xaa];
        let pps = vec![0x68, 0xce, 0x3c, 0x80];
        let idr: Vec<u8> = std::iter::once(0x65).chain((0..5000).map(|i| (i % 251) as u8 | 1)).collect();
        let input = au(&[sps.clone(), pps.clone(), idr.clone()]);
        let mut pk = H264Packetizer::new();
        let out = roundtrip(&input, &mut pk, 1100);
        assert_eq!(out, input);
        let pl = pk.packetize(&input, 1100).unwrap();
        assert_eq!(pl[0][0] & 0x1f, STAP_A);
        assert_eq!(pl[1][0] & 0x1f, FU_A);
    }

    #[test]
    fn sps_pps_repeated_on_bare_idr() {
        let sps = vec![0x67, 0x42, 0xe0, 0x1f];
        let pps = vec![0x68, 0xce];
        let idr = vec![0x65, 1, 2, 3];
        let mut pk = H264Packetizer::new();
        roundtrip(&au(&[sps.clone(), pps.clone(), idr.clone()]), &mut pk, 1100);
        let p = au(&[vec![0x41, 9, 9]]);
        assert_eq!(roundtrip(&p, &mut pk, 1100), p);
        let out = roundtrip(&au(std::slice::from_ref(&idr)), &mut pk, 1100);
        assert_eq!(out, au(&[sps, pps, idr]));
    }

    #[test]
    fn aud_stripped_and_split() {
        let input = [0, 0, 0, 1, 0x09, 0xf0, 0, 0, 1, 0x41, 5, 6, 0];
        let nals = split_annex_b(&input);
        assert_eq!(nals, vec![&[0x09, 0xf0][..], &[0x41, 5, 6][..]]);
        let mut pk = H264Packetizer::new();
        assert_eq!(pk.packetize(&input, 1100).unwrap(), vec![vec![0x41, 5, 6]]);
    }
}
