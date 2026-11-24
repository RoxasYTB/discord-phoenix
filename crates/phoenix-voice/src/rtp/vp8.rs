use super::{Depacketizer, Packetizer};
use crate::{Error, Result};

#[derive(Default)]
pub struct Vp8Packetizer {
    picture_id: u16,
}

impl Packetizer for Vp8Packetizer {
    fn packetize(&mut self, frame: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>> {
        if max_payload <= 4 {
            return Err(Error::Malformed("mtu too small"));
        }
        if frame.is_empty() {
            return Err(Error::Malformed("empty vp8 frame"));
        }
        let pid = self.picture_id & 0x7fff;
        self.picture_id = self.picture_id.wrapping_add(1);
        Ok(frame
            .chunks(max_payload - 4)
            .enumerate()
            .map(|(i, c)| {
                let mut p = Vec::with_capacity(c.len() + 4);
                p.push(0x80 | if i == 0 { 0x10 } else { 0 });
                p.push(0x80);
                p.push(0x80 | (pid >> 8) as u8);
                p.push(pid as u8);
                p.extend_from_slice(c);
                p
            })
            .collect())
    }
}

pub fn descriptor_len(p: &[u8]) -> Result<usize> {
    let b0 = *p.first().ok_or(Error::Malformed("vp8"))?;
    let mut n = 1;
    if b0 & 0x80 != 0 {
        let x = *p.get(1).ok_or(Error::Malformed("vp8 x"))?;
        n += 1;
        if x & 0x80 != 0 {
            let m = *p.get(n).ok_or(Error::Malformed("vp8 pid"))?;
            n += if m & 0x80 != 0 { 2 } else { 1 };
        }
        if x & 0x40 != 0 {
            n += 1;
        }
        if x & 0x30 != 0 {
            n += 1;
        }
    }
    if n > p.len() {
        return Err(Error::Malformed("vp8 descriptor"));
    }
    Ok(n)
}

pub fn is_keyframe(frame: &[u8]) -> bool {
    frame.first().is_some_and(|b| b & 1 == 0)
}

#[derive(Default)]
pub struct Vp8Depacketizer {
    buf: Vec<u8>,
}

impl Depacketizer for Vp8Depacketizer {
    fn push(&mut self, p: &[u8], marker: bool) -> Result<Option<Vec<u8>>> {
        let n = descriptor_len(p)?;
        if p[0] & 0x10 != 0 {
            self.buf.clear();
        }
        self.buf.extend_from_slice(&p[n..]);
        Ok(marker.then(|| std::mem::take(&mut self.buf)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
        let mut pk = Vp8Packetizer { picture_id: 0x7fff };
        let pl = pk.packetize(&f, 1000).unwrap();
        assert_eq!(pl.len(), 4);
        assert_eq!(pl[0][0], 0x90);
        assert_eq!(pl[1][0], 0x80);
        let mut d = Vp8Depacketizer::default();
        let mut out = None;
        for (i, p) in pl.iter().enumerate() {
            out = d.push(p, i + 1 == pl.len()).unwrap();
        }
        assert_eq!(out.unwrap(), f);
        assert_eq!(pk.picture_id, 0x8000);
    }
}
