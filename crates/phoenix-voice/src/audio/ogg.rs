use std::sync::OnceLock;

pub const OPUS_RATE: u32 = 48_000;
pub const PRE_SKIP: u16 = 312;
const VENDOR: &str = "discord-phoenix";
const PACKETS_PER_PAGE: usize = 50;

fn crc_table() -> &'static [u32; 256] {
    static T: OnceLock<[u32; 256]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut r = (i as u32) << 24;
            for _ in 0..8 {
                r = if r & 0x8000_0000 != 0 { (r << 1) ^ 0x04c1_1db7 } else { r << 1 };
            }
            *e = r;
        }
        t
    })
}

pub fn crc(data: &[u8]) -> u32 {
    let t = crc_table();
    data.iter().fold(0u32, |c, &b| (c << 8) ^ t[(((c >> 24) as u8) ^ b) as usize])
}

fn lacing(len: usize) -> Vec<u8> {
    let mut v = vec![255u8; len / 255];
    v.push((len % 255) as u8);
    v
}

struct Writer {
    out: Vec<u8>,
    serial: u32,
    seq: u32,
}

impl Writer {
    fn page(&mut self, packets: &[&[u8]], granule: u64, header_type: u8) {
        let segments: Vec<u8> = packets.iter().flat_map(|p| lacing(p.len())).collect();
        let start = self.out.len();
        self.out.extend_from_slice(b"OggS");
        self.out.push(0);
        self.out.push(header_type);
        self.out.extend_from_slice(&granule.to_le_bytes());
        self.out.extend_from_slice(&self.serial.to_le_bytes());
        self.out.extend_from_slice(&self.seq.to_le_bytes());
        self.out.extend_from_slice(&[0; 4]);
        self.out.push(segments.len() as u8);
        self.out.extend_from_slice(&segments);
        for p in packets {
            self.out.extend_from_slice(p);
        }
        let c = crc(&self.out[start..]);
        self.out[start + 22..start + 26].copy_from_slice(&c.to_le_bytes());
        self.seq += 1;
    }
}

pub fn opus_head(channels: u8, pre_skip: u16, input_rate: u32) -> Vec<u8> {
    let mut h = b"OpusHead".to_vec();
    h.push(1);
    h.push(channels);
    h.extend_from_slice(&pre_skip.to_le_bytes());
    h.extend_from_slice(&input_rate.to_le_bytes());
    h.extend_from_slice(&0i16.to_le_bytes());
    h.push(0);
    h
}

pub fn opus_tags() -> Vec<u8> {
    let mut t = b"OpusTags".to_vec();
    t.extend_from_slice(&(VENDOR.len() as u32).to_le_bytes());
    t.extend_from_slice(VENDOR.as_bytes());
    t.extend_from_slice(&0u32.to_le_bytes());
    t
}

pub fn write_opus(packets: &[Vec<u8>], frame_samples: u64, total_samples: u64, serial: u32) -> Vec<u8> {
    let mut w = Writer { out: Vec::with_capacity(packets.iter().map(Vec::len).sum::<usize>() + 4096), serial, seq: 0 };
    w.page(&[&opus_head(1, PRE_SKIP, OPUS_RATE)], 0, 0x02);
    w.page(&[&opus_tags()], 0, 0);
    let end = u64::from(PRE_SKIP) + total_samples;
    if packets.is_empty() {
        w.page(&[], end, 0x04);
        return w.out;
    }
    let mut chunks: Vec<&[Vec<u8>]> = Vec::new();
    let (mut start, mut segs) = (0usize, 0usize);
    for (i, p) in packets.iter().enumerate() {
        let n = p.len() / 255 + 1;
        if i > start && (segs + n > 255 || i - start >= PACKETS_PER_PAGE) {
            chunks.push(&packets[start..i]);
            start = i;
            segs = 0;
        }
        segs += n;
    }
    chunks.push(&packets[start..]);
    let mut done = 0u64;
    for (i, chunk) in chunks.iter().enumerate() {
        done += chunk.len() as u64;
        let last = i + 1 == chunks.len();
        let granule = if last { end } else { (u64::from(PRE_SKIP) + done * frame_samples).min(end) };
        let refs: Vec<&[u8]> = chunk.iter().map(Vec::as_slice).collect();
        w.page(&refs, granule, if last { 0x04 } else { 0 });
    }
    w.out
}

pub const WEBCODECS_SERIAL: u32 = 0x0100_0000;

pub fn write_webcodecs(packets: &[Vec<u8>], samples_per_packet: u64, channels: u8, input_rate: u32) -> Vec<u8> {
    let mut w = Writer { out: Vec::with_capacity(packets.iter().map(Vec::len).sum::<usize>() + 4096), serial: WEBCODECS_SERIAL, seq: 0 };
    w.page(&[&opus_head(channels, 0, input_rate)], 0, 0x02);
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&[0; 8]);
    w.page(&[&tags], 0, 0);
    let mut granule = 0u64;
    for p in packets {
        granule += samples_per_packet;
        w.page(&[p], granule, 0);
    }
    w.page(&[], granule, 0x04);
    w.out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpusStream {
    pub channels: u8,
    pub pre_skip: u16,
    pub packets: Vec<Vec<u8>>,
    pub last_granule: u64,
}

impl OpusStream {
    pub fn duration_samples(&self) -> u64 {
        self.last_granule.saturating_sub(u64::from(self.pre_skip))
    }
}

pub fn read_opus(data: &[u8]) -> Option<OpusStream> {
    let mut packets: Vec<Vec<u8>> = Vec::new();
    let mut partial: Vec<u8> = Vec::new();
    let mut last_granule = 0u64;
    let mut pos = 0usize;
    while pos + 27 <= data.len() {
        if &data[pos..pos + 4] != b"OggS" {
            return None;
        }
        let granule = u64::from_le_bytes(data[pos + 6..pos + 14].try_into().ok()?);
        let n = data[pos + 26] as usize;
        let seg_start = pos + 27;
        let body_start = seg_start + n;
        if body_start > data.len() {
            return None;
        }
        let segs = &data[seg_start..body_start];
        let body_len: usize = segs.iter().map(|&s| s as usize).sum();
        if body_start + body_len > data.len() {
            return None;
        }
        let mut off = body_start;
        for &s in segs {
            partial.extend_from_slice(&data[off..off + s as usize]);
            off += s as usize;
            if s < 255 {
                packets.push(std::mem::take(&mut partial));
            }
        }
        if granule != u64::MAX {
            last_granule = granule;
        }
        pos = body_start + body_len;
    }
    let mut it = packets.into_iter();
    let head = it.next()?;
    if head.len() < 19 || &head[..8] != b"OpusHead" {
        return None;
    }
    let channels = head[9].max(1);
    let pre_skip = u16::from_le_bytes([head[10], head[11]]);
    let tags = it.next()?;
    if tags.len() < 8 || &tags[..8] != b"OpusTags" {
        return None;
    }
    Some(OpusStream { channels, pre_skip, packets: it.filter(|p| !p.is_empty()).collect(), last_granule })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_ogg_reference() {
        assert_eq!(crc(b""), 0);
        assert_eq!(crc(b"123456789"), 0x89a1_897f);
    }

    #[test]
    fn write_then_read_roundtrip() {
        let packets: Vec<Vec<u8>> = (0..123u32).map(|i| vec![(i % 251) as u8; 40 + (i as usize % 300)]).collect();
        let total = 123 * 960 - 100;
        let file = write_opus(&packets, 960, total, 7);
        assert_eq!(&file[..4], b"OggS");
        assert_eq!(file[5], 0x02);
        let s = read_opus(&file).expect("parse");
        assert_eq!(s.channels, 1);
        assert_eq!(s.pre_skip, PRE_SKIP);
        assert_eq!(s.packets, packets);
        assert_eq!(s.duration_samples(), total);
    }

    #[test]
    fn page_checksums_are_valid() {
        let file = write_opus(&[vec![1, 2, 3], vec![4; 600]], 960, 1920, 1);
        let mut pos = 0;
        let mut pages = 0;
        while pos < file.len() {
            let n = file[pos + 26] as usize;
            let body: usize = file[pos + 27..pos + 27 + n].iter().map(|&s| s as usize).sum();
            let end = pos + 27 + n + body;
            let mut page = file[pos..end].to_vec();
            let stored = u32::from_le_bytes(page[22..26].try_into().unwrap());
            page[22..26].fill(0);
            assert_eq!(crc(&page), stored);
            pos = end;
            pages += 1;
        }
        assert_eq!(pages, 3);
        assert_eq!(file[file.len() - (27 + 1 + 3 + 3 + 600)..][5], 0x04);
    }

    #[test]
    fn big_packets_split_pages_under_255_segments() {
        let packets: Vec<Vec<u8>> = (0..60).map(|_| vec![9u8; 1275]).collect();
        let file = write_opus(&packets, 960, 60 * 960, 3);
        let mut pos = 0;
        while pos < file.len() {
            let n = file[pos + 26] as usize;
            let body: usize = file[pos + 27..pos + 27 + n].iter().map(|&s| s as usize).sum();
            pos += 27 + n + body;
        }
        assert_eq!(pos, file.len());
        assert_eq!(read_opus(&file).unwrap().packets, packets);
    }

    #[test]
    fn rejects_garbage() {
        assert!(read_opus(b"not an ogg file at all, definitely not").is_none());
    }
}
