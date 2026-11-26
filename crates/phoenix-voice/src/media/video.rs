use super::{VideoEncoder, VideoEncoderFactory};
use crate::codec::{Codec, VideoEncoderConfig};
use crate::{Error, Result};

pub fn probe_capabilities() -> crate::codec::LocalCapabilities {
    let ffmpeg = super::ffmpeg::available();
    crate::codec::LocalCapabilities {
        hw_av1_encoder: false,
        h264_encoder: ffmpeg,
        vp8_encoder: false,
        decoders: if ffmpeg { super::ffmpeg::decoder_support().codecs.clone() } else { Vec::new() },
    }
}

#[derive(Default)]
pub struct SystemEncoderFactory;

impl VideoEncoderFactory for SystemEncoderFactory {
    fn create(&self, config: &VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>> {
        if !config.codec.sendable() {
            return Err(Error::Unsupported(format!("{} is never used for sending", config.codec.name())));
        }
        match config.codec {
            Codec::H264 if super::ffmpeg::available() => Ok(Box::new(super::ffmpeg::FfmpegH264::spawn(config)?)),
            c => Err(Error::Unsupported(format!("no {} encoder available", c.name()))),
        }
    }
}
