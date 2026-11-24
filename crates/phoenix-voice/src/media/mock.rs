use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::{EncodedVideo, VideoEncoder, VideoEncoderFactory, VideoFrame};
use crate::codec::{Codec, VideoEncoderConfig};
use crate::{Error, Result};

pub struct MockEncoder {
    codec: Codec,
    frames: usize,
    fail_after: Option<usize>,
    pub bitrate: u32,
}

impl MockEncoder {
    pub fn new(codec: Codec, fail_after: Option<usize>) -> Self {
        Self { codec, frames: 0, fail_after, bitrate: 0 }
    }
}

impl VideoEncoder for MockEncoder {
    fn codec(&self) -> Codec {
        self.codec
    }

    fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>> {
        if self.fail_after.is_some_and(|n| self.frames >= n) {
            return Err(Error::Encoder(format!("mock {} encoder failure", self.codec.name())));
        }
        let key = force_keyframe || self.frames == 0;
        self.frames += 1;
        let body: Vec<u8> = (0..2500u32).map(|i| (i % 250) as u8 + 2).collect();
        let data = match self.codec {
            Codec::H264 => {
                let mut v = Vec::new();
                if key && self.frames == 1 {
                    v.extend_from_slice(&[0, 0, 0, 1, 0x67, 0x42, 0xe0, 0x1f, 0, 0, 0, 1, 0x68, 0xce, 0x3c, 0x80]);
                }
                v.extend_from_slice(&[0, 0, 0, 1, if key { 0x65 } else { 0x41 }]);
                v.extend_from_slice(&body);
                v
            }
            Codec::Vp8 => {
                let mut v = vec![if key { 0x10 } else { 0x11 }];
                v.extend_from_slice(&body);
                v
            }
            Codec::Av1 => {
                let mut v = Vec::new();
                if key {
                    v.extend_from_slice(&[0x0a, 0x03, 0, 0, 0]);
                }
                v.push(0x32);
                crate::leb128::write(body.len() as u64, &mut v);
                v.extend_from_slice(&body);
                v
            }
            c => return Err(Error::Unsupported(format!("mock cannot encode {}", c.name()))),
        };
        Ok(Some(EncodedVideo { codec: self.codec, keyframe: key, data, pts_us: frame.pts_us }))
    }

    fn set_bitrate(&mut self, bps: u32) -> Result<()> {
        self.bitrate = bps;
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct MockEncoderFactory {
    pub unavailable: Vec<Codec>,
    pub fail_after: Vec<(Codec, usize)>,
    pub created: Arc<AtomicUsize>,
}

impl VideoEncoderFactory for MockEncoderFactory {
    fn create(&self, config: &VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>> {
        if self.unavailable.contains(&config.codec) || !config.codec.sendable() {
            return Err(Error::Unsupported(format!("{} unavailable", config.codec.name())));
        }
        self.created.fetch_add(1, Ordering::SeqCst);
        let fail = self.fail_after.iter().find(|(c, _)| *c == config.codec).map(|(_, n)| *n);
        Ok(Box::new(MockEncoder::new(config.codec, fail)))
    }
}
