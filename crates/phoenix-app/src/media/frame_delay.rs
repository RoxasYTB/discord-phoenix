pub const MIN_MS: u32 = 10;
pub const DEFAULT_MS: u32 = 100;

pub fn normalize(bytes: &mut [u8]) -> usize {
    if bytes.starts_with(b"GIF8") {
        gif_delays(bytes).unwrap_or(0)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        webp_delays(bytes).unwrap_or(0)
    } else {
        0
    }
}

fn skip_sub_blocks(b: &[u8], mut i: usize) -> Option<usize> {
    loop {
        let n = *b.get(i)? as usize;
        i += 1;
        if n == 0 {
            return Some(i);
        }
        i += n;
    }
}

fn color_table_len(flags: u8) -> usize {
    if flags & 0x80 != 0 { 3 << ((flags & 7) + 1) } else { 0 }
}

fn gif_delays(b: &mut [u8]) -> Option<usize> {
    let mut i = 13 + color_table_len(*b.get(10)?);
    let mut fixed = 0;
    loop {
        match *b.get(i)? {
            0x21 => {
                if *b.get(i + 1)? == 0xF9 && *b.get(i + 2)? == 4 {
                    let cs = u16::from_le_bytes([*b.get(i + 4)?, *b.get(i + 5)?]) as u32;
                    if cs * 10 <= MIN_MS {
                        let v = ((DEFAULT_MS / 10) as u16).to_le_bytes();
                        b[i + 4] = v[0];
                        b[i + 5] = v[1];
                        fixed += 1;
                    }
                }
                i = skip_sub_blocks(b, i + 2)?;
            }
            0x2C => {
                let flags = *b.get(i + 9)?;
                i = skip_sub_blocks(b, i + 10 + color_table_len(flags) + 1)?;
            }
            _ => return Some(fixed),
        }
    }
}

fn webp_delays(b: &mut [u8]) -> Option<usize> {
    let mut i = 12;
    let mut fixed = 0;
    while i + 8 <= b.len() {
        let size = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        if &b[i..i + 4] == b"ANMF" {
            let d = i + 8 + 12;
            let ms = u32::from_le_bytes([*b.get(d)?, *b.get(d + 1)?, *b.get(d + 2)?, 0]);
            if ms <= MIN_MS {
                let v = DEFAULT_MS.to_le_bytes();
                b[d..d + 3].copy_from_slice(&v[..3]);
                fixed += 1;
            }
        }
        i += 8 + size + (size & 1);
    }
    Some(fixed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;

    fn gif_with(delays_ms: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = image::codecs::gif::GifEncoder::new(&mut out);
            enc.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();
            let frames = delays_ms.iter().enumerate().map(|(n, d)| {
                let px = image::Rgba([(n % 2) as u8 * 255, 0, 0, 255]);
                image::Frame::from_parts(image::RgbaImage::from_pixel(2, 2, px), 0, 0, image::Delay::from_numer_denom_ms(*d, 1))
            });
            enc.encode_frames(frames).unwrap();
        }
        out
    }

    fn decoded_delays(bytes: &[u8]) -> Vec<u32> {
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).unwrap();
        dec.into_frames().map(|f| f.unwrap().delay().numer_denom_ms()).map(|(n, d)| n / d).collect()
    }

    #[test]
    fn short_gif_delays_play_at_100ms_like_chromium() {
        let mut b = gif_with(&[0, 10, 20, 50, 100]);
        assert_eq!(normalize(&mut b), 2);
        assert_eq!(decoded_delays(&b), vec![100, 100, 20, 50, 100]);
    }

    #[test]
    fn other_bytes_are_untouched() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\0".to_vec();
        let before = png.clone();
        assert_eq!(normalize(&mut png), 0);
        assert_eq!(png, before);
        let mut cut = gif_with(&[0, 0]);
        cut.truncate(cut.len() / 2);
        normalize(&mut cut);
    }

    #[test]
    fn short_webp_frame_durations_are_raised() {
        let mut anmf = vec![0u8; 16];
        anmf[12..15].copy_from_slice(&[5, 0, 0]);
        let mut long = vec![0u8; 16];
        long[12..15].copy_from_slice(&[40, 0, 0]);
        let mut b = b"RIFF\0\0\0\0WEBP".to_vec();
        for p in [&anmf, &long] {
            b.extend_from_slice(b"ANMF");
            b.extend_from_slice(&(p.len() as u32).to_le_bytes());
            b.extend_from_slice(p);
        }
        assert_eq!(normalize(&mut b), 1);
        assert_eq!(&b[12 + 8 + 12..12 + 8 + 15], &[100, 0, 0]);
        assert_eq!(&b[12 + 24 + 8 + 12..12 + 24 + 8 + 15], &[40, 0, 0]);
    }
}
