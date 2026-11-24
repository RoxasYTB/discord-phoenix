use super::{VideoEncoder, VideoEncoderFactory};
use crate::codec::{Codec, VideoEncoderConfig};
use crate::{Error, Result};

pub fn probe_capabilities() -> crate::codec::LocalCapabilities {
    crate::codec::LocalCapabilities {
        hw_av1_encoder: false,
        h264_encoder: cfg!(any(feature = "ffmpeg", feature = "vaapi", feature = "nvenc")),
        vp8_encoder: cfg!(feature = "ffmpeg"),
        decoders: vec![Codec::H264, Codec::Vp8, Codec::Av1],
    }
}

#[derive(Default)]
pub struct SystemEncoderFactory;

impl VideoEncoderFactory for SystemEncoderFactory {
    fn create(&self, config: &VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>> {
        if !config.codec.sendable() {
            return Err(Error::Unsupported(format!("{} is never used for sending", config.codec.name())));
        }
        Err(Error::Unsupported(format!(
            "no {} encoder backend compiled in (enable ffmpeg/vaapi/nvenc)",
            config.codec.name()
        )))
    }
}
