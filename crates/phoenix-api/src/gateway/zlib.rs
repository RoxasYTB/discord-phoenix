use flate2::{Decompress, FlushDecompress, Status};

const SUFFIX: [u8; 4] = [0x00, 0x00, 0xFF, 0xFF];

pub struct ZlibStream {
    inflater: Decompress,
    pending: Vec<u8>,
    out: Vec<u8>,
}

impl Default for ZlibStream {
    fn default() -> Self {
        Self::new()
    }
}

impl ZlibStream {
    pub fn new() -> Self {
        Self { inflater: Decompress::new(true), pending: Vec::new(), out: Vec::with_capacity(64 * 1024) }
    }

    pub fn push(&mut self, frame: &[u8]) -> Result<Option<String>, String> {
        self.pending.extend_from_slice(frame);
        if self.pending.len() < 4 || self.pending[self.pending.len() - 4..] != SUFFIX {
            return Ok(None);
        }
        self.out.clear();
        let mut input = &self.pending[..];
        let mut buf = [0u8; 32 * 1024];
        loop {
            let before_in = self.inflater.total_in();
            let before_out = self.inflater.total_out();
            let status = self
                .inflater
                .decompress(input, &mut buf, FlushDecompress::Sync)
                .map_err(|e| e.to_string())?;
            let consumed = (self.inflater.total_in() - before_in) as usize;
            let produced = (self.inflater.total_out() - before_out) as usize;
            self.out.extend_from_slice(&buf[..produced]);
            input = &input[consumed..];
            if matches!(status, Status::StreamEnd) || (input.is_empty() && produced < buf.len()) {
                break;
            }
            if consumed == 0 && produced == 0 {
                return Err("zlib stream stalled".into());
            }
        }
        self.pending.clear();
        String::from_utf8(std::mem::take(&mut self.out))
            .map(Some)
            .map_err(|e| e.to_string())
    }
}

pub struct ZlibEncoder {
    deflater: flate2::Compress,
}

impl Default for ZlibEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ZlibEncoder {
    pub fn new() -> Self {
        Self { deflater: flate2::Compress::new(flate2::Compression::default(), true) }
    }

    pub fn encode(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() / 2 + 64);
        let mut input = data;
        loop {
            out.reserve(4096);
            let before_in = self.deflater.total_in();
            self.deflater
                .compress_vec(input, &mut out, flate2::FlushCompress::Sync)
                .expect("deflate");
            let consumed = (self.deflater.total_in() - before_in) as usize;
            input = &input[consumed..];
            if input.is_empty() && out.len() < out.capacity() {
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_split_and_sequential_messages() {
        let mut enc = ZlibEncoder::new();
        let mut dec = ZlibStream::new();
        let a = enc.encode(br#"{"op":10,"d":{"heartbeat_interval":41250}}"#);
        assert_eq!(&a[a.len() - 4..], &SUFFIX);
        let (x, y) = a.split_at(a.len() / 2);
        assert_eq!(dec.push(x).unwrap(), None);
        assert_eq!(dec.push(y).unwrap().unwrap(), r#"{"op":10,"d":{"heartbeat_interval":41250}}"#);
        let big = format!(r#"{{"t":"X","d":"{}"}}"#, "abc".repeat(50_000));
        let b = enc.encode(big.as_bytes());
        assert_eq!(dec.push(&b).unwrap().unwrap(), big);
        let c = enc.encode(b"{}");
        assert_eq!(dec.push(&c).unwrap().unwrap(), "{}");
    }

    #[test]
    fn garbage_errors() {
        let mut dec = ZlibStream::new();
        assert!(dec.push(&[1, 2, 3, 4, 0, 0, 0xFF, 0xFF]).is_err());
    }
}
