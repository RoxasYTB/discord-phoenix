pub mod audio;
pub mod codec;
pub mod connection;
pub mod crypto;
pub mod dave;
pub mod error;
pub mod gateway;
#[cfg(target_os = "linux")]
pub mod hotkey;
#[cfg(windows)]
pub mod win_hotkey;
pub mod leb128;
pub mod media;
pub mod rtp;
#[cfg(target_os = "linux")]
pub mod screencast;
pub mod session;
pub mod sink_wants;
pub mod udp;
pub mod video_rx;

pub use error::{Error, Result};

pub const FRAME_SAMPLES: usize = 960;
