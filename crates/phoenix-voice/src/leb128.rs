use crate::{Error, Result};

pub fn write(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

pub fn encoded_len(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

pub fn read(data: &[u8]) -> Result<(u64, usize)> {
    let mut v: u64 = 0;
    for (i, b) in data.iter().enumerate().take(10) {
        v |= ((b & 0x7f) as u64) << (7 * i);
        if b & 0x80 == 0 {
            return Ok((v, i + 1));
        }
    }
    Err(Error::Malformed("leb128"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn roundtrip() {
        for v in [0u64, 1, 127, 128, 300, 16384, u32::MAX as u64, u64::MAX] {
            let mut b = Vec::new();
            super::write(v, &mut b);
            assert_eq!(b.len(), super::encoded_len(v));
            assert_eq!(super::read(&b).unwrap(), (v, b.len()));
        }
        let mut b = Vec::new();
        super::write(300, &mut b);
        assert_eq!(b, [0xac, 0x02]);
    }
}
