pub mod camera;
#[cfg(windows)]
pub mod desktop;
pub mod ffmpeg;
pub mod mock;
pub mod testsrc;
#[cfg(feature = "opus")]
pub mod opus;
#[cfg(feature = "opus")]
pub mod soundshare;
pub mod video;

use crate::codec::{Codec, VideoEncoderConfig};
use crate::Result;

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pts_us: u64,
    pub data: std::sync::Arc<[u8]>,
}

#[derive(Debug, Clone)]
pub struct EncodedVideo {
    pub codec: Codec,
    pub keyframe: bool,
    pub data: Vec<u8>,
    pub pts_us: u64,
}

pub trait VideoEncoder: Send {
    fn codec(&self) -> Codec;
    fn encode(&mut self, frame: &VideoFrame, force_keyframe: bool) -> Result<Option<EncodedVideo>>;
    fn set_bitrate(&mut self, bps: u32) -> Result<()>;
}

pub trait VideoEncoderFactory: Send + Sync {
    fn create(&self, config: &VideoEncoderConfig) -> Result<Box<dyn VideoEncoder>>;
}

pub trait AudioEncoder: Send {
    fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>>;
    fn set_bitrate(&mut self, bps: u32) -> Result<()>;
}

pub trait AudioDecoder: Send {
    fn decode(&mut self, packet: Option<&[u8]>) -> Result<Vec<i16>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Camera,
    Screen,
    Microphone,
}

pub trait CaptureSource: Send {
    fn kind(&self) -> SourceKind;
    fn next_video_frame(&mut self) -> Result<Option<VideoFrame>> {
        Ok(None)
    }
    fn next_audio_frame(&mut self) -> Result<Option<Vec<i16>>> {
        Ok(None)
    }
    fn stop(&mut self) -> Result<()>;
}
